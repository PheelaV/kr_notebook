"""Command-line interface for database management."""

import os
import shutil
import sqlite3
from datetime import datetime
from pathlib import Path

import click
import pyrootutils

# Find project root (contains Cargo.toml)
ROOT = pyrootutils.find_root(search_from=__file__, indicator="Cargo.toml")
DATA_DIR = ROOT / "data"
PRODUCTION_DB = DATA_DIR / "hangul.db"
GOLDEN_DB = DATA_DIR / "reference" / "golden.db"  # Clean reference for scenarios
BACKUPS_DIR = DATA_DIR / "backups"
SCENARIOS_DIR = DATA_DIR / "scenarios"
CONFIG_TOML = ROOT / "config.toml"


def ensure_dirs() -> None:
    """Ensure backup and scenario directories exist."""
    BACKUPS_DIR.mkdir(parents=True, exist_ok=True)
    SCENARIOS_DIR.mkdir(parents=True, exist_ok=True)


def get_active_db() -> Path:
    """Get the currently active database path from config."""
    # Priority 1: config.toml
    if CONFIG_TOML.exists():
        import tomllib

        with open(CONFIG_TOML, "rb") as f:
            try:
                config = tomllib.load(f)
                if path := config.get("database", {}).get("path"):
                    return ROOT / path
            except Exception:
                pass

    # Priority 2: .env
    env_file = ROOT / ".env"
    if env_file.exists():
        with open(env_file) as f:
            for line in f:
                line = line.strip()
                if line.startswith("DATABASE_PATH="):
                    path = line.split("=", 1)[1].strip().strip('"').strip("'")
                    return ROOT / path

    # Default
    return PRODUCTION_DB


def resolve_db_path(name: str) -> Path | None:
    """Resolve a database name to a full path."""
    if name == "production":
        return PRODUCTION_DB

    # Check scenarios
    scenario_db = SCENARIOS_DIR / f"{name}.db"
    if scenario_db.exists():
        return scenario_db

    # Check backups
    for backup in BACKUPS_DIR.glob("*.db"):
        if backup.stem == name or backup.name == name:
            return backup

    return None


@click.group()
@click.version_option()
def cli() -> None:
    """Database management for kr_notebook.

    Manage production database, create backups, and switch between
    test scenarios for development and testing.
    """
    pass


@cli.command()
@click.option(
    "--name",
    "-n",
    type=str,
    default=None,
    help="Custom backup name (default: timestamp).",
)
def backup(name: str | None) -> None:
    """Create a backup of the current database.

    Creates a timestamped copy of the active database in data/backups/.
    Uses SQLite VACUUM INTO for a clean, compact backup.
    """
    ensure_dirs()

    active_db = get_active_db()
    if not active_db.exists():
        raise click.ClickException(f"Database not found: {active_db}")

    if name:
        backup_name = f"{name}.db"
    else:
        timestamp = datetime.now().strftime("%Y%m%d_%H%M%S")
        backup_name = f"{timestamp}_hangul.db"

    backup_path = BACKUPS_DIR / backup_name

    if backup_path.exists():
        raise click.ClickException(f"Backup already exists: {backup_path}")

    click.echo(f"Backing up: {active_db}")
    click.echo(f"       To: {backup_path}")

    # Use VACUUM INTO for a clean backup
    conn = sqlite3.connect(active_db)
    try:
        conn.execute(f"VACUUM INTO '{backup_path}'")
        click.echo(click.style("Backup created successfully!", fg="green"))

        # Show backup size
        size_mb = backup_path.stat().st_size / (1024 * 1024)
        click.echo(f"Size: {size_mb:.2f} MB")
    finally:
        conn.close()


# Scenario presets: name -> (description, apply_function)
# Each apply function takes (conn, echo) and modifies the database
SCENARIO_PRESETS: dict[str, tuple[str, callable]] = {}


def _register_preset(name: str, description: str):
    """Decorator to register a scenario preset."""
    def decorator(func):
        SCENARIO_PRESETS[name] = (description, func)
        return func
    return decorator


# SQL to reset a card to pristine "new" state (all SRS fields to defaults)
RESET_CARD_SQL = """
    ease_factor = 2.5,
    interval_days = 0,
    repetitions = 0,
    next_review = datetime('now'),
    total_reviews = 0,
    correct_reviews = 0,
    learning_step = 0,
    fsrs_stability = NULL,
    fsrs_difficulty = NULL,
    fsrs_state = 'New'
"""

# SQL to set a card to "graduated" state (learned, ready for long-term review)
GRADUATED_CARD_SQL = """
    ease_factor = 2.5,
    interval_days = 7,
    repetitions = 5,
    next_review = datetime('now', '+7 days'),
    total_reviews = 10,
    correct_reviews = 5,
    learning_step = 4,
    fsrs_stability = 7.0,
    fsrs_difficulty = 5.0,
    fsrs_state = 'Review'
"""


@_register_preset("tier3_fresh", "Tier 2 complete, tier 3 ready to unlock")
def _apply_tier3_fresh(conn: sqlite3.Connection, echo: callable) -> None:
    echo("Setting tier 1 & 2 to graduated, tier 3 to new...")
    # Tier 1 & 2 fully learned
    conn.execute(f"UPDATE cards SET {GRADUATED_CARD_SQL} WHERE tier IN (1, 2)")
    # Tier 3 & 4 fresh
    conn.execute(f"UPDATE cards SET {RESET_CARD_SQL} WHERE tier IN (3, 4)")
    # Set max unlocked tier to 2 (tier 3 will unlock on next visit)
    conn.execute("INSERT OR REPLACE INTO settings (key, value) VALUES ('max_unlocked_tier', '2')")
    # Clear focus mode
    conn.execute("DELETE FROM settings WHERE key = 'focus_tier'")
    conn.commit()
    echo("Tier 3 will unlock on next homepage/study visit")


@_register_preset("tier3_unlock", "Tier 3 at 80% (tier unlock threshold)")
def _apply_tier3_unlock(conn: sqlite3.Connection, echo: callable) -> None:
    total = conn.execute("SELECT COUNT(*) FROM cards WHERE tier = 3").fetchone()[0]
    learn_count = int(total * 80 / 100)
    echo(f"Setting tier 3 to 80% learned ({learn_count}/{total} cards)...")

    # Reset all tier 3 cards first
    conn.execute(f"UPDATE cards SET {RESET_CARD_SQL} WHERE tier = 3")

    # Graduate first 80%
    conn.execute(f"""
        UPDATE cards SET {GRADUATED_CARD_SQL}
        WHERE tier = 3 AND id IN (
            SELECT id FROM cards WHERE tier = 3 ORDER BY id LIMIT {learn_count}
        )
    """)
    conn.commit()


@cli.command("create-scenario")
@click.argument("preset", required=False)
@click.option(
    "--list",
    "-l",
    "list_presets",
    is_flag=True,
    help="List available scenario presets.",
)
def create_scenario(preset: str | None, list_presets: bool) -> None:
    """Create a test scenario from a preset.

    Copies the production database and applies the preset modifications.

    Examples:

        # List available presets
        db-manager create-scenario --list

        # Create tier 3 fresh scenario
        db-manager create-scenario tier3_fresh

        # Create scenario for testing tier unlock
        db-manager create-scenario tier3_unlock
    """
    ensure_dirs()

    if list_presets:
        click.echo(click.style("Available presets:", bold=True))
        for name, (desc, _) in SCENARIO_PRESETS.items():
            click.echo(f"  {name:15} - {desc}")
        return

    if preset is None:
        raise click.ClickException("PRESET is required (or use --list)")

    if preset not in SCENARIO_PRESETS:
        raise click.ClickException(
            f"Unknown preset: {preset}\n"
            f"Run 'db-manager create-scenario --list' to see available presets."
        )

    # Use golden reference database as source
    if not GOLDEN_DB.exists():
        raise click.ClickException(
            f"Golden database not found: {GOLDEN_DB}\n"
            f"Create it by running the server once to seed cards, then reset to clean state."
        )

    scenario_path = SCENARIOS_DIR / f"{preset}.db"
    if scenario_path.exists():
        if not click.confirm(f"Scenario '{preset}' exists. Overwrite?"):
            click.echo("Aborted.")
            return

    # Copy golden database (clean slate with all cards in new state)
    click.echo(f"Creating scenario: {preset}")
    shutil.copy2(GOLDEN_DB, scenario_path)

    # Apply preset
    desc, apply_fn = SCENARIO_PRESETS[preset]
    conn = sqlite3.connect(scenario_path)
    try:
        apply_fn(conn, click.echo)
        click.echo(click.style(f"Scenario created: {scenario_path}", fg="green"))
        click.echo()
        click.echo(f"To use: db-manager use {preset}")
    finally:
        conn.close()


@cli.command("list")
def list_dbs() -> None:
    """List all databases (production, backups, scenarios)."""
    ensure_dirs()

    active_db = get_active_db()
    click.echo(click.style("=== Databases ===", bold=True))
    click.echo()

    # Production
    click.echo(click.style("Production:", bold=True))
    if PRODUCTION_DB.exists():
        size_mb = PRODUCTION_DB.stat().st_size / (1024 * 1024)
        is_active = " (active)" if active_db == PRODUCTION_DB else ""
        click.echo(f"  hangul.db - {size_mb:.2f} MB{click.style(is_active, fg='green')}")
    else:
        click.echo("  (not found)")
    click.echo()

    # Scenarios
    click.echo(click.style("Scenarios:", bold=True))
    scenarios = sorted(SCENARIOS_DIR.glob("*.db"))
    if scenarios:
        for db in scenarios:
            size_mb = db.stat().st_size / (1024 * 1024)
            is_active = " (active)" if active_db == db else ""
            click.echo(f"  {db.stem} - {size_mb:.2f} MB{click.style(is_active, fg='green')}")
    else:
        click.echo("  (none)")
    click.echo()

    # Backups
    click.echo(click.style("Backups:", bold=True))
    backups = sorted(BACKUPS_DIR.glob("*.db"), reverse=True)
    if backups:
        for db in backups[:10]:  # Show last 10
            size_mb = db.stat().st_size / (1024 * 1024)
            mtime = datetime.fromtimestamp(db.stat().st_mtime)
            is_active = " (active)" if active_db == db else ""
            click.echo(f"  {db.stem} - {size_mb:.2f} MB - {mtime:%Y-%m-%d %H:%M}{click.style(is_active, fg='green')}")
        if len(backups) > 10:
            click.echo(f"  ... and {len(backups) - 10} more")
    else:
        click.echo("  (none)")


@cli.command()
@click.argument("name")
def use(name: str) -> None:
    """Switch to a different database.

    Updates config.toml to point to the specified database.
    Use 'production' to switch back to the main database.

    Examples:

        db-manager use tier3_fresh   # Use a scenario
        db-manager use production    # Back to production
    """
    db_path = resolve_db_path(name)
    if db_path is None:
        raise click.ClickException(
            f"Database not found: {name}\n"
            f"Run 'db-manager list' to see available databases."
        )

    # Calculate relative path from project root
    try:
        rel_path = db_path.relative_to(ROOT)
    except ValueError:
        rel_path = db_path

    # Update config.toml
    if name == "production":
        # Remove database section from config.toml (preserves comments/formatting)
        if CONFIG_TOML.exists():
            import tomlkit

            with open(CONFIG_TOML, "r") as f:
                config = tomlkit.load(f)

            if "database" in config:
                del config["database"]
                with open(CONFIG_TOML, "w") as f:
                    tomlkit.dump(config, f)
                click.echo("Removed database override from config.toml")
            else:
                click.echo("No database override in config.toml")
        click.echo(click.style(f"Switched to production database", fg="green"))
    else:
        # Write config.toml with database path
        config_content = f'''[database]
path = "{rel_path}"
'''
        with open(CONFIG_TOML, "w") as f:
            f.write(config_content)
        click.echo(click.style(f"Switched to: {name}", fg="green"))
        click.echo(f"Path: {rel_path}")

    click.echo()
    click.echo("Restart the server to use the new database.")


@cli.command()
@click.argument("name")
@click.option(
    "--yes",
    "-y",
    is_flag=True,
    help="Skip confirmation prompt.",
)
def restore(name: str, yes: bool) -> None:
    """Restore production database from a backup.

    Replaces the production database with the specified backup.
    Creates a safety backup before restoring.
    """
    # Find the backup
    backup_path = None
    for backup in BACKUPS_DIR.glob("*.db"):
        if backup.stem == name or backup.name == name:
            backup_path = backup
            break

    if backup_path is None:
        raise click.ClickException(
            f"Backup not found: {name}\n"
            f"Run 'db-manager list' to see available backups."
        )

    if not yes:
        click.echo(f"This will replace the production database with: {backup_path.name}")
        if not click.confirm("Continue?"):
            click.echo("Aborted.")
            return

    # Safety backup
    ensure_dirs()
    safety_name = f"pre_restore_{datetime.now().strftime('%Y%m%d_%H%M%S')}.db"
    safety_path = BACKUPS_DIR / safety_name

    if PRODUCTION_DB.exists():
        click.echo(f"Creating safety backup: {safety_name}")
        shutil.copy2(PRODUCTION_DB, safety_path)

    # Restore
    click.echo(f"Restoring from: {backup_path.name}")
    shutil.copy2(backup_path, PRODUCTION_DB)
    click.echo(click.style("Restore complete!", fg="green"))

    # Switch back to production if using a scenario
    if CONFIG_TOML.exists():
        os.remove(CONFIG_TOML)
        click.echo("Removed config.toml override (now using production)")

    click.echo()
    click.echo("Restart the server to use the restored database.")


@cli.command()
def info() -> None:
    """Show current database statistics."""
    active_db = get_active_db()

    if not active_db.exists():
        raise click.ClickException(f"Database not found: {active_db}")

    click.echo(click.style("=== Database Info ===", bold=True))
    click.echo()
    click.echo(f"Active database: {active_db}")
    size_mb = active_db.stat().st_size / (1024 * 1024)
    click.echo(f"Size: {size_mb:.2f} MB")
    click.echo()

    conn = sqlite3.connect(active_db)
    try:
        # Card counts by tier
        click.echo(click.style("Cards by Tier:", bold=True))
        for tier in range(1, 5):
            row = conn.execute("""
                SELECT
                    COUNT(*) as total,
                    SUM(CASE WHEN total_reviews = 0 THEN 1 ELSE 0 END) as new,
                    SUM(CASE WHEN repetitions >= 2 THEN 1 ELSE 0 END) as learned
                FROM cards WHERE tier = ?
            """, (tier,)).fetchone()
            total, new, learned = row
            pct = int(learned * 100 / total) if total > 0 else 0
            click.echo(f"  Tier {tier}: {learned}/{total} learned ({pct}%), {new} new")

        click.echo()

        # Total stats
        row = conn.execute("""
            SELECT
                COUNT(*) as total,
                SUM(total_reviews) as reviews,
                SUM(CASE WHEN repetitions >= 2 THEN 1 ELSE 0 END) as learned
            FROM cards
        """).fetchone()
        total, reviews, learned = row
        click.echo(click.style("Totals:", bold=True))
        click.echo(f"  Cards: {total}")
        click.echo(f"  Learned: {learned}")
        click.echo(f"  Total reviews: {reviews}")

        # Settings
        click.echo()
        click.echo(click.style("Settings:", bold=True))
        settings = conn.execute("SELECT key, value FROM settings").fetchall()
        for key, value in settings:
            click.echo(f"  {key}: {value}")

    finally:
        conn.close()
