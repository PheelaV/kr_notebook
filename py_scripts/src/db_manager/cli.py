"""Command-line interface for database management.

Updated for multi-user architecture:
- Auth DB: data/app.db (users, sessions, app_settings)
- User DBs: data/users/{username}/learning.db (per-user cards, progress, settings)
"""

import os
import secrets
import shutil
import sqlite3
from datetime import datetime, timedelta
from pathlib import Path

import click
import pyrootutils

# Find project root (contains Cargo.toml)
ROOT = pyrootutils.find_root(search_from=__file__, indicator="Cargo.toml")
DATA_DIR = ROOT / "data"

# Multi-user paths
AUTH_DB = DATA_DIR / "app.db"
USERS_DIR = DATA_DIR / "users"

# Legacy single-user path (for backwards compatibility)
LEGACY_DB = DATA_DIR / "hangul.db"

# Reference and test data paths
REFERENCE_DIR = DATA_DIR / "reference"
GOLDEN_AUTH_DB = REFERENCE_DIR / "golden_auth.db"
GOLDEN_LEARNING_DB = REFERENCE_DIR / "golden_learning.db"
BACKUPS_DIR = DATA_DIR / "backups"
SCENARIOS_DIR = DATA_DIR / "scenarios"

CONFIG_TOML = ROOT / "config.toml"


def ensure_dirs() -> None:
    """Ensure required directories exist."""
    BACKUPS_DIR.mkdir(parents=True, exist_ok=True)
    SCENARIOS_DIR.mkdir(parents=True, exist_ok=True)
    REFERENCE_DIR.mkdir(parents=True, exist_ok=True)
    USERS_DIR.mkdir(parents=True, exist_ok=True)


def get_user_db_path(username: str, data_dir: Path | None = None) -> Path:
    """Get the learning database path for a user."""
    users_base = (Path(data_dir) / "users") if data_dir else USERS_DIR
    return users_base / username / "learning.db"


def get_user_scenario_dir(username: str, data_dir: Path | None = None) -> Path:
    """Get the scenario directory for a user."""
    scenarios_base = (Path(data_dir) / "scenarios") if data_dir else SCENARIOS_DIR
    return scenarios_base / username


def get_app_db_path(data_dir: Path | None = None) -> Path:
    """Get app.db path for given data directory."""
    return (Path(data_dir) / "app.db") if data_dir else AUTH_DB


def user_exists_in_env(username: str, data_dir: Path | None = None) -> bool:
    """Check if a user exists in the auth database (environment-aware)."""
    app_db = get_app_db_path(data_dir)
    if not app_db.exists():
        return False
    conn = sqlite3.connect(app_db)
    try:
        count = conn.execute(
            "SELECT COUNT(*) FROM users WHERE username = ?", (username,)
        ).fetchone()[0]
        return count > 0
    finally:
        conn.close()


def user_exists(username: str) -> bool:
    """Check if a user exists in the auth database (uses default DATA_DIR)."""
    return user_exists_in_env(username, None)


def list_all_users(data_dir: Path | None = None) -> list[tuple[str, bool, str | None]]:
    """List all users: (username, is_guest, last_login_at)."""
    app_db = get_app_db_path(data_dir)
    if not app_db.exists():
        return []
    conn = sqlite3.connect(app_db)
    try:
        rows = conn.execute(
            "SELECT username, COALESCE(is_guest, 0), last_login_at FROM users ORDER BY username"
        ).fetchall()
        return [(r[0], bool(r[1]), r[2]) for r in rows]
    finally:
        conn.close()


def hash_password_for_storage(password: str, username: str) -> str:
    """Hash password for storage, matching the browser→server flow.

    The web app uses two-stage hashing:
    1. Client computes SHA256(password:username)
    2. Server applies Argon2 to the SHA256 hash

    This function replicates both stages for CLI user creation.
    """
    import hashlib

    # Stage 1: Client-side SHA256 (password:username)
    # Note: Browser's auth.js uses username.toLowerCase() at line 85
    client_hash = hashlib.sha256(f"{password}:{username.lower()}".encode()).hexdigest()

    # Stage 2: Server-side Argon2
    try:
        from argon2 import PasswordHasher
        ph = PasswordHasher()
        return ph.hash(client_hash)
    except ImportError:
        raise click.ClickException(
            "argon2-cffi is required for password hashing. "
            "Install with: pip install argon2-cffi"
        )


# ==================== CLI Groups ====================

@click.group()
@click.version_option()
def cli() -> None:
    """Database management for kr_notebook (multi-user).

    Manage auth database, per-user learning databases, create backups,
    and switch between test scenarios for development and testing.

    Most commands require a --user parameter to specify which user's
    database to operate on.
    """
    pass


# ==================== User Management ====================

@cli.command("create-user")
@click.argument("username")
@click.option(
    "--password",
    "-p",
    type=str,
    default=None,
    help="Password (default: random generated).",
)
@click.option(
    "--guest",
    is_flag=True,
    help="Create as guest user.",
)
def create_user(username: str, password: str | None, guest: bool) -> None:
    """Create a new user with auth entry and learning database.

    Creates:
    - Entry in data/app.db (auth database)
    - Directory data/users/{username}/
    - Learning database data/users/{username}/learning.db with seeded cards

    Example:
        db-manager create-user alice
        db-manager create-user bob --password secret123
        db-manager create-user temp --guest
    """
    ensure_dirs()

    if user_exists(username):
        raise click.ClickException(f"User already exists: {username}")

    # Generate password if not provided
    if password is None:
        password = secrets.token_urlsafe(12)
        click.echo(f"Generated password: {click.style(password, fg='cyan')}")

    # Create user directory
    user_dir = USERS_DIR / username
    user_dir.mkdir(parents=True, exist_ok=True)

    # Initialize auth database if needed
    if not AUTH_DB.exists():
        click.echo(f"Creating auth database: {AUTH_DB}")
        init_auth_db(AUTH_DB)

    # Add user to auth database
    password_hash = hash_password_for_storage(password, username)
    now = datetime.now().isoformat()

    conn = sqlite3.connect(AUTH_DB)
    try:
        conn.execute(
            """INSERT INTO users (username, password_hash, created_at, is_guest, last_activity_at)
               VALUES (?, ?, ?, ?, ?)""",
            (username, password_hash, now, 1 if guest else 0, now),
        )
        conn.commit()
        click.echo(f"Added user to auth database: {username}")
    finally:
        conn.close()

    # Create learning database using Rust CLI (ensures schema matches server)
    user_db_path = get_user_db_path(username)
    import subprocess

    result = subprocess.run(
        ["cargo", "run", "--release", "--", "--init-user-db", username],
        cwd=ROOT,
        capture_output=True,
        text=True,
    )
    if result.returncode != 0:
        raise click.ClickException(
            f"Failed to initialize learning.db: {result.stderr}"
        )
    click.echo(f"Created learning database: {user_db_path}")

    click.echo(click.style(f"User '{username}' created successfully!", fg="green"))


@cli.command("delete-user")
@click.argument("username")
@click.option(
    "--yes",
    "-y",
    is_flag=True,
    help="Skip confirmation prompt.",
)
@click.option(
    "--data-dir",
    "-d",
    type=click.Path(),
    default=None,
    help="Data directory (for test environments).",
)
def delete_user(username: str, yes: bool, data_dir: str | None) -> None:
    """Delete a user and all their data.

    Removes:
    - Entry from app.db
    - Directory users/{username}/ (including learning.db)
    - Any scenarios

    Example:
        db-manager delete-user alice
        db-manager delete-user bob --yes
        db-manager delete-user _test_user --data-dir data/test/auth --yes
    """
    # Determine paths based on data-dir option
    if data_dir:
        env_dir = Path(data_dir)
        app_db = env_dir / "app.db"
        users_base = env_dir / "users"
        scenarios_base = env_dir / "scenarios"
    else:
        app_db = AUTH_DB
        users_base = USERS_DIR
        scenarios_base = SCENARIOS_DIR

    if not app_db.exists():
        raise click.ClickException(f"Database not found: {app_db}")

    # Check if user exists
    conn = sqlite3.connect(app_db)
    try:
        exists = conn.execute(
            "SELECT COUNT(*) FROM users WHERE username = ?",
            (username,)
        ).fetchone()[0]
    finally:
        conn.close()

    if not exists:
        raise click.ClickException(f"User not found: {username}")

    if not yes:
        click.echo(f"This will delete user '{username}' and ALL their data:")
        click.echo(f"  - Auth entry in {app_db}")
        click.echo(f"  - User directory {users_base / username}")
        click.echo(f"  - Scenarios in {scenarios_base / username}")
        if not click.confirm("Continue?"):
            click.echo("Aborted.")
            return

    # Delete from auth database
    conn = sqlite3.connect(app_db)
    try:
        conn.execute("DELETE FROM users WHERE username = ?", (username,))
        conn.commit()
        click.echo(f"Removed from auth database: {username}")
    finally:
        conn.close()

    # Delete user directory
    user_dir = users_base / username
    if user_dir.exists():
        shutil.rmtree(user_dir)
        click.echo(f"Deleted user directory: {user_dir}")

    # Delete scenarios
    scenario_dir = scenarios_base / username
    if scenario_dir.exists():
        shutil.rmtree(scenario_dir)
        click.echo(f"Deleted scenarios: {scenario_dir}")

    click.echo(click.style(f"User '{username}' deleted.", fg="green"))


@cli.command("list-users")
@click.option(
    "--data-dir",
    "-d",
    type=click.Path(),
    default=None,
    help="Data directory (for test environments).",
)
def list_users_cmd(data_dir: str | None) -> None:
    """List all users in the auth database.

    Shows username, type (regular/guest), and last login time.
    """
    env_dir = Path(data_dir) if data_dir else None
    app_db = get_app_db_path(env_dir)

    if not app_db.exists():
        click.echo("No auth database found. No users exist yet.")
        return

    users = list_all_users(env_dir)
    if not users:
        click.echo("No users found.")
        return

    click.echo(click.style("=== Users ===", bold=True))
    click.echo()

    for username, is_guest, last_login in users:
        user_type = click.style("guest", fg="yellow") if is_guest else "user"
        login_str = last_login[:19] if last_login else "never"
        user_db = get_user_db_path(username, env_dir)
        db_exists = click.style("OK", fg="green") if user_db.exists() else click.style("MISSING", fg="red")

        click.echo(f"  {username:15} [{user_type:5}] last_login: {login_str}  db: {db_exists}")


# ==================== Info Command ====================

@cli.command()
@click.option(
    "--user",
    "-u",
    type=str,
    default=None,
    help="Username to show info for (required for user-specific info).",
)
def info(user: str | None) -> None:
    """Show database statistics.

    Without --user: shows auth database summary
    With --user: shows user's learning database details

    Examples:
        db-manager info              # Auth DB summary
        db-manager info --user alice # Alice's learning stats
    """
    ensure_dirs()

    if user is None:
        # Show auth database summary
        _show_auth_info()
    else:
        # Show user-specific info
        if not user_exists(user):
            raise click.ClickException(f"User not found: {user}")
        _show_user_info(user)


def _show_auth_info() -> None:
    """Show auth database summary."""
    click.echo(click.style("=== Auth Database Info ===", bold=True))
    click.echo()

    if not AUTH_DB.exists():
        click.echo(f"Auth database not found: {AUTH_DB}")
        return

    click.echo(f"Database: {AUTH_DB}")
    size_kb = AUTH_DB.stat().st_size / 1024
    click.echo(f"Size: {size_kb:.1f} KB")
    click.echo()

    conn = sqlite3.connect(AUTH_DB)
    try:
        # User counts
        total = conn.execute("SELECT COUNT(*) FROM users").fetchone()[0]
        regular = conn.execute("SELECT COUNT(*) FROM users WHERE COALESCE(is_guest, 0) = 0").fetchone()[0]
        guests = conn.execute("SELECT COUNT(*) FROM users WHERE is_guest = 1").fetchone()[0]

        click.echo(click.style("Users:", bold=True))
        click.echo(f"  Total: {total}")
        click.echo(f"  Regular: {regular}")
        click.echo(f"  Guests: {guests}")
        click.echo()

        # Session counts
        active_sessions = conn.execute(
            "SELECT COUNT(*) FROM sessions WHERE expires_at > datetime('now')"
        ).fetchone()[0]
        click.echo(click.style("Sessions:", bold=True))
        click.echo(f"  Active: {active_sessions}")

    finally:
        conn.close()


def _show_user_info(username: str) -> None:
    """Show user's learning database info."""
    click.echo(click.style(f"=== User Info: {username} ===", bold=True))
    click.echo()

    user_db = get_user_db_path(username)
    if not user_db.exists():
        click.echo(f"Learning database not found: {user_db}")
        return

    click.echo(f"Database: {user_db}")
    size_kb = user_db.stat().st_size / 1024
    click.echo(f"Size: {size_kb:.1f} KB")
    click.echo()

    conn = sqlite3.connect(user_db)
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
            if total > 0:
                pct = int(learned * 100 / total)
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
        click.echo(f"  Total reviews: {reviews or 0}")

        # Settings
        click.echo()
        click.echo(click.style("Settings:", bold=True))
        settings = conn.execute("SELECT key, value FROM settings").fetchall()
        for key, value in settings:
            click.echo(f"  {key}: {value}")

    finally:
        conn.close()


# ==================== Scenario Presets ====================

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

# Scenario presets: name -> (description, apply_function)
SCENARIO_PRESETS: dict[str, tuple[str, callable]] = {}


def _register_preset(name: str, description: str):
    """Decorator to register a scenario preset."""
    def decorator(func):
        SCENARIO_PRESETS[name] = (description, func)
        return func
    return decorator


@_register_preset("tier1_new", "Fresh start, tier 1 only, no reviews")
def _apply_tier1_new(conn: sqlite3.Connection, echo: callable) -> None:
    echo("Resetting all cards to new state...")
    conn.execute(f"UPDATE cards SET {RESET_CARD_SQL}")
    conn.execute("INSERT OR REPLACE INTO settings (key, value) VALUES ('max_unlocked_tier', '1')")
    conn.execute("DELETE FROM settings WHERE key = 'focus_tier'")
    conn.execute("DELETE FROM settings WHERE key = 'all_tiers_unlocked'")
    conn.commit()


@_register_preset("tier3_fresh", "Tiers 1-2 graduated, tier 3 unlocked but new")
def _apply_tier3_fresh(conn: sqlite3.Connection, echo: callable) -> None:
    echo("Setting tier 1 & 2 to graduated, tier 3 to new...")
    conn.execute(f"UPDATE cards SET {GRADUATED_CARD_SQL} WHERE tier IN (1, 2)")
    conn.execute(f"UPDATE cards SET {RESET_CARD_SQL} WHERE tier IN (3, 4)")
    conn.execute("INSERT OR REPLACE INTO settings (key, value) VALUES ('max_unlocked_tier', '3')")
    conn.execute("DELETE FROM settings WHERE key = 'focus_tier'")
    conn.commit()


@_register_preset("tier3_unlock", "Tier 3 at 80% (about to unlock tier 4)")
def _apply_tier3_unlock(conn: sqlite3.Connection, echo: callable) -> None:
    total = conn.execute("SELECT COUNT(*) FROM cards WHERE tier = 3").fetchone()[0]
    learn_count = int(total * 80 / 100)
    echo(f"Setting tier 3 to 80% learned ({learn_count}/{total} cards)...")

    conn.execute(f"UPDATE cards SET {GRADUATED_CARD_SQL} WHERE tier IN (1, 2)")
    conn.execute(f"UPDATE cards SET {RESET_CARD_SQL} WHERE tier = 3")
    conn.execute(f"""
        UPDATE cards SET {GRADUATED_CARD_SQL}
        WHERE tier = 3 AND id IN (
            SELECT id FROM cards WHERE tier = 3 ORDER BY id LIMIT {learn_count}
        )
    """)
    conn.execute(f"UPDATE cards SET {RESET_CARD_SQL} WHERE tier = 4")
    conn.execute("INSERT OR REPLACE INTO settings (key, value) VALUES ('max_unlocked_tier', '3')")
    conn.commit()


@_register_preset("all_graduated", "All tiers unlocked and graduated")
def _apply_all_graduated(conn: sqlite3.Connection, echo: callable) -> None:
    echo("Setting all cards to graduated state...")

    # Update legacy cards table (for backwards compatibility)
    conn.execute(f"UPDATE cards SET {GRADUATED_CARD_SQL}")

    # Insert graduated state into card_progress for all baseline cards (IDs 1-80)
    # The Rust app uses card_progress for SRS state, not the legacy cards table
    graduated_sql = """
        INSERT OR REPLACE INTO card_progress (
            card_id, ease_factor, interval_days, repetitions, next_review,
            total_reviews, correct_reviews, learning_step,
            fsrs_stability, fsrs_difficulty, fsrs_state
        ) VALUES (
            ?, 2.5, 7, 5, datetime('now', '+7 days'),
            10, 5, 4, 7.0, 5.0, 'Review'
        )
    """
    # Insert for all 80 baseline Hangul cards (IDs are 1-80 from cargo run --init-db)
    for card_id in range(1, 81):
        conn.execute(graduated_sql, (card_id,))

    conn.execute("INSERT OR REPLACE INTO settings (key, value) VALUES ('max_unlocked_tier', '4')")
    # Note: NOT enabling all_tiers_unlocked to avoid accelerated mode showing unreviewed-today cards
    conn.execute("INSERT OR REPLACE INTO settings (key, value) VALUES ('all_tiers_unlocked', 'false')")
    conn.commit()
    echo(f"Inserted graduated state for 80 cards into card_progress")


# ==================== Scenario Commands ====================

@cli.command("create-scenario")
@click.argument("preset", required=False)
@click.option(
    "--user",
    "-u",
    type=str,
    required=False,
    help="Username to create scenario for.",
)
@click.option(
    "--list",
    "-l",
    "list_presets",
    is_flag=True,
    help="List available scenario presets.",
)
@click.option(
    "--data-dir",
    "-d",
    type=click.Path(),
    default=None,
    help="Data directory (for test environments).",
)
def create_scenario(preset: str | None, user: str | None, list_presets: bool, data_dir: str | None) -> None:
    """Create a test scenario from a preset.

    Creates a modified copy of the user's learning database for testing.
    Scenarios are stored in {data_dir}/scenarios/{username}/{preset}.db

    Examples:
        db-manager create-scenario --list
        db-manager create-scenario --user alice tier3_fresh
        db-manager create-scenario --user bob tier1_new
        db-manager create-scenario --user bob tier1_new --data-dir data/test/e2e
    """
    ensure_dirs()

    if list_presets:
        click.echo(click.style("Available presets:", bold=True))
        for name, (desc, _) in SCENARIO_PRESETS.items():
            click.echo(f"  {name:15} - {desc}")
        return

    if preset is None:
        raise click.ClickException("PRESET is required (or use --list)")

    if user is None:
        raise click.ClickException("--user is required for creating scenarios")

    if preset not in SCENARIO_PRESETS:
        raise click.ClickException(
            f"Unknown preset: {preset}\n"
            f"Run 'db-manager create-scenario --list' to see available presets."
        )

    # Convert data_dir to Path if provided
    env_dir = Path(data_dir) if data_dir else None

    if not user_exists_in_env(user, env_dir):
        raise click.ClickException(f"User not found: {user}")

    # Source: user's current learning database or golden reference
    source_db = get_user_db_path(user, env_dir)
    if not source_db.exists():
        # Try golden reference
        if GOLDEN_LEARNING_DB.exists():
            source_db = GOLDEN_LEARNING_DB
            click.echo(f"Using golden reference (user DB not found)")
        else:
            raise click.ClickException(
                f"No source database found.\n"
                f"Either create user's learning.db or provide a golden reference."
            )

    # Create scenario directory for user
    scenario_dir = get_user_scenario_dir(user, env_dir)
    scenario_dir.mkdir(parents=True, exist_ok=True)

    scenario_path = scenario_dir / f"{preset}.db"
    if scenario_path.exists():
        if not click.confirm(f"Scenario '{preset}' exists for {user}. Overwrite?"):
            click.echo("Aborted.")
            return

    # Copy source database
    click.echo(f"Creating scenario: {user}/{preset}")
    shutil.copy2(source_db, scenario_path)

    # Apply preset
    desc, apply_fn = SCENARIO_PRESETS[preset]
    conn = sqlite3.connect(scenario_path)
    try:
        apply_fn(conn, click.echo)
        click.echo(click.style(f"Scenario created: {scenario_path}", fg="green"))
        click.echo()
        data_dir_arg = f" --data-dir {data_dir}" if data_dir else ""
        click.echo(f"To use: db-manager use --user {user} {preset}{data_dir_arg}")
    finally:
        conn.close()


@cli.command("use")
@click.argument("name")
@click.option(
    "--user",
    "-u",
    type=str,
    required=True,
    help="Username to switch database for.",
)
@click.option(
    "--data-dir",
    "-d",
    type=click.Path(),
    default=None,
    help="Data directory (for test environments).",
)
def use_scenario(name: str, user: str, data_dir: str | None) -> None:
    """Switch a user's database to a scenario or back to production.

    Copies the scenario database over the user's learning.db.
    Creates a backup of the current database first.

    Examples:
        db-manager use --user alice tier3_fresh   # Use scenario
        db-manager use --user alice production    # Restore original
        db-manager use --user alice tier1_new --data-dir data/test/e2e
    """
    # Convert data_dir to Path if provided
    env_dir = Path(data_dir) if data_dir else None

    if not user_exists_in_env(user, env_dir):
        raise click.ClickException(f"User not found: {user}")

    user_db = get_user_db_path(user, env_dir)
    scenario_dir = get_user_scenario_dir(user, env_dir)

    # Backup directory within the environment
    backup_base = (Path(data_dir) / "backups") if data_dir else BACKUPS_DIR
    backup_dir = backup_base / user
    backup_dir.mkdir(parents=True, exist_ok=True)

    if name == "production":
        # Restore from backup
        backup_path = backup_dir / "pre_scenario.db"
        if not backup_path.exists():
            raise click.ClickException(
                f"No production backup found for {user}.\n"
                f"The 'production' option restores from a backup created when switching to a scenario."
            )

        click.echo(f"Restoring {user}'s production database...")
        shutil.copy2(backup_path, user_db)
        click.echo(click.style(f"Restored production database for {user}", fg="green"))
        return

    # Find scenario
    scenario_path = scenario_dir / f"{name}.db"
    if not scenario_path.exists():
        raise click.ClickException(
            f"Scenario not found: {user}/{name}\n"
            f"Available scenarios in {scenario_dir}:\n" +
            "\n".join(f"  - {p.stem}" for p in scenario_dir.glob("*.db"))
            if scenario_dir.exists() else "  (none)"
        )

    # Backup current database
    if user_db.exists():
        backup_path = backup_dir / "pre_scenario.db"
        click.echo(f"Backing up current database to: {backup_path}")
        shutil.copy2(user_db, backup_path)

    # Copy scenario
    click.echo(f"Switching {user} to scenario: {name}")
    shutil.copy2(scenario_path, user_db)
    click.echo(click.style(f"Switched to scenario: {name}", fg="green"))
    click.echo()
    click.echo("Restart the server to use the new database state.")


# ==================== Apply Preset Command ====================


@cli.command("apply-preset")
@click.argument("preset")
@click.option(
    "--user",
    "-u",
    type=str,
    required=True,
    help="Username to apply preset to.",
)
@click.option(
    "--data-dir",
    "-d",
    type=click.Path(),
    default=None,
    help="Data directory (for test environments).",
)
def apply_preset(preset: str, user: str, data_dir: str | None) -> None:
    """Apply a scenario preset directly to a user's learning database.

    Unlike 'use' which switches to a pre-created scenario file, this command
    applies the preset transformations directly to the user's learning.db.
    Useful for E2E tests that need to set up scenarios on-the-fly.

    Examples:
        db-manager apply-preset tier1_new --user alice
        db-manager apply-preset tier3_fresh --user bob --data-dir data/test/e2e
    """
    # Convert data_dir to Path if provided
    env_dir = Path(data_dir).resolve() if data_dir else None

    if not user_exists_in_env(user, env_dir):
        raise click.ClickException(f"User not found: {user}")

    if preset not in SCENARIO_PRESETS:
        raise click.ClickException(
            f"Unknown preset: {preset}\n"
            f"Available: {', '.join(SCENARIO_PRESETS.keys())}"
        )

    user_db = get_user_db_path(user, env_dir)
    if not user_db.exists():
        raise click.ClickException(f"Learning database not found: {user_db}")

    desc, apply_fn = SCENARIO_PRESETS[preset]
    conn = sqlite3.connect(user_db)
    try:
        apply_fn(conn, click.echo)
        click.echo(click.style(f"Applied preset '{preset}' to {user}", fg="green"))
    finally:
        conn.close()


# ==================== List Command ====================

@cli.command("list")
@click.option(
    "--user",
    "-u",
    type=str,
    default=None,
    help="Show scenarios for specific user.",
)
def list_dbs(user: str | None) -> None:
    """List databases and scenarios.

    Without --user: shows all users and their databases
    With --user: shows scenarios for that user
    """
    ensure_dirs()

    if user is not None:
        _list_user_scenarios(user)
    else:
        _list_all()


def _list_all() -> None:
    """List all users and their databases."""
    click.echo(click.style("=== Database Overview ===", bold=True))
    click.echo()

    # Auth database
    click.echo(click.style("Auth Database:", bold=True))
    if AUTH_DB.exists():
        size_kb = AUTH_DB.stat().st_size / 1024
        click.echo(f"  {AUTH_DB.name} - {size_kb:.1f} KB")
    else:
        click.echo("  (not found)")
    click.echo()

    # Users
    click.echo(click.style("Users:", bold=True))
    users = list_all_users()
    if users:
        for username, is_guest, _ in users:
            user_db = get_user_db_path(username)
            scenario_dir = get_user_scenario_dir(username)

            user_type = "(guest)" if is_guest else ""
            if user_db.exists():
                size_kb = user_db.stat().st_size / 1024
                db_status = f"{size_kb:.1f} KB"
            else:
                db_status = click.style("MISSING", fg="red")

            scenario_count = len(list(scenario_dir.glob("*.db"))) if scenario_dir.exists() else 0
            scenario_str = f", {scenario_count} scenarios" if scenario_count > 0 else ""

            click.echo(f"  {username:15} {user_type:8} db: {db_status}{scenario_str}")
    else:
        click.echo("  (no users)")
    click.echo()

    # Global backups
    click.echo(click.style("Backups:", bold=True))
    backup_count = len(list(BACKUPS_DIR.glob("**/*.db"))) if BACKUPS_DIR.exists() else 0
    click.echo(f"  {backup_count} backup files in {BACKUPS_DIR}")


def _list_user_scenarios(username: str) -> None:
    """List scenarios for a specific user."""
    if not user_exists(username):
        raise click.ClickException(f"User not found: {username}")

    click.echo(click.style(f"=== Scenarios for {username} ===", bold=True))
    click.echo()

    scenario_dir = get_user_scenario_dir(username)
    if not scenario_dir.exists() or not list(scenario_dir.glob("*.db")):
        click.echo("  (no scenarios)")
        click.echo()
        click.echo("Create one with: db-manager create-scenario --user " + username + " <preset>")
        return

    for db in sorted(scenario_dir.glob("*.db")):
        size_kb = db.stat().st_size / 1024
        mtime = datetime.fromtimestamp(db.stat().st_mtime)
        click.echo(f"  {db.stem:15} - {size_kb:.1f} KB - {mtime:%Y-%m-%d %H:%M}")


# ==================== Backup Command ====================

@cli.command("backup")
@click.option(
    "--user",
    "-u",
    type=str,
    required=True,
    help="Username to backup.",
)
@click.option(
    "--name",
    "-n",
    type=str,
    default=None,
    help="Custom backup name (default: timestamp).",
)
def backup(user: str, name: str | None) -> None:
    """Create a backup of a user's learning database.

    Creates a timestamped copy in data/backups/{username}/.
    Uses SQLite VACUUM INTO for a clean, compact backup.

    Examples:
        db-manager backup --user alice
        db-manager backup --user alice --name before_experiment
    """
    ensure_dirs()

    if not user_exists(user):
        raise click.ClickException(f"User not found: {user}")

    user_db = get_user_db_path(user)
    if not user_db.exists():
        raise click.ClickException(f"Learning database not found: {user_db}")

    backup_dir = BACKUPS_DIR / user
    backup_dir.mkdir(parents=True, exist_ok=True)

    if name:
        backup_name = f"{name}.db"
    else:
        timestamp = datetime.now().strftime("%Y%m%d_%H%M%S")
        backup_name = f"{timestamp}.db"

    backup_path = backup_dir / backup_name

    if backup_path.exists():
        raise click.ClickException(f"Backup already exists: {backup_path}")

    click.echo(f"Backing up: {user_db}")
    click.echo(f"       To: {backup_path}")

    conn = sqlite3.connect(user_db)
    try:
        conn.execute(f"VACUUM INTO '{backup_path}'")
        click.echo(click.style("Backup created successfully!", fg="green"))

        size_kb = backup_path.stat().st_size / 1024
        click.echo(f"Size: {size_kb:.1f} KB")
    finally:
        conn.close()


# ==================== Database Initialization ====================

def init_auth_db(db_path: Path) -> None:
    """Initialize an empty auth database with schema."""
    conn = sqlite3.connect(db_path)
    try:
        conn.executescript("""
            CREATE TABLE IF NOT EXISTS users (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                username TEXT NOT NULL UNIQUE COLLATE NOCASE,
                password_hash TEXT NOT NULL,
                created_at TEXT NOT NULL,
                last_login_at TEXT,
                is_guest INTEGER DEFAULT 0,
                last_activity_at TEXT
            );

            CREATE TABLE IF NOT EXISTS sessions (
                id TEXT PRIMARY KEY,
                user_id INTEGER NOT NULL,
                created_at TEXT NOT NULL,
                expires_at TEXT NOT NULL,
                last_access_at TEXT NOT NULL,
                FOREIGN KEY (user_id) REFERENCES users(id) ON DELETE CASCADE
            );

            CREATE TABLE IF NOT EXISTS app_settings (
                key TEXT PRIMARY KEY,
                value TEXT
            );

            CREATE INDEX IF NOT EXISTS idx_sessions_user_id ON sessions(user_id);
            CREATE INDEX IF NOT EXISTS idx_sessions_expires_at ON sessions(expires_at);
            CREATE INDEX IF NOT EXISTS idx_users_is_guest ON users(is_guest);

            INSERT OR IGNORE INTO app_settings (key, value) VALUES ('max_users', NULL);
            INSERT OR IGNORE INTO app_settings (key, value) VALUES ('max_guests', NULL);
            INSERT OR IGNORE INTO app_settings (key, value) VALUES ('guest_expiry_hours', '24');
        """)
        conn.commit()
    finally:
        conn.close()


# NOTE: Learning database schema is now handled by Rust CLI (--init-user-db)
# to ensure schema matches exactly what the server expects. See:
# - src/db/schema.rs for the canonical schema
# - main.rs --init-user-db for user db initialization


# ==================== Test Environment Commands ====================

def get_test_env_dir(name: str) -> Path:
    """Get the path to a test environment directory."""
    return DATA_DIR / "test" / name


@cli.command("init-test-env")
@click.argument("name")
@click.option(
    "--data-dir",
    "-d",
    type=click.Path(),
    default=None,
    help="Override data directory path (default: data/test/<name>).",
)
@click.option(
    "--skip-build",
    is_flag=True,
    help="Skip cargo build (assumes binary is already built).",
)
def init_test_env(name: str, data_dir: str | None, skip_build: bool) -> None:
    """Initialize a fresh test environment with full schema.

    Creates an isolated data directory with:
    - app.db with full v0→v9 schema (using Rust --init-db)
    - users/ directory for per-user databases
    - 80 baseline Hangul cards seeded

    Uses the Rust server's --init-db flag to ensure schema matches
    exactly what the server expects. This avoids schema duplication.

    Use this before running E2E tests to ensure a clean slate.

    Examples:
        db-manager init-test-env auth-tests
        db-manager init-test-env study-tests --data-dir /tmp/test_data
        db-manager init-test-env quick-test --skip-build
    """
    import subprocess

    # Determine environment directory (resolve to absolute path for cargo)
    if data_dir:
        env_dir = Path(data_dir).resolve()
    else:
        env_dir = get_test_env_dir(name).resolve()

    # Check if environment already exists
    if env_dir.exists():
        if not click.confirm(f"Environment '{name}' exists at {env_dir}. Overwrite?"):
            click.echo("Aborted.")
            return
        shutil.rmtree(env_dir)

    # Create environment directory structure
    env_dir.mkdir(parents=True, exist_ok=True)
    users_dir = env_dir / "users"
    users_dir.mkdir(exist_ok=True)

    click.echo(f"Creating test environment: {env_dir}")

    # Build the Rust binary if needed
    if not skip_build:
        click.echo("  - Building Rust server...")
        result = subprocess.run(
            ["cargo", "build", "--quiet"],
            cwd=ROOT,
            capture_output=True,
            text=True,
        )
        if result.returncode != 0:
            click.echo(click.style(f"Build failed: {result.stderr}", fg="red"))
            raise click.ClickException("Failed to build Rust server")

    # Initialize database using Rust --init-db flag
    click.echo("  - Initializing app.db with v0→v9 schema...")
    env = os.environ.copy()
    env["DATA_DIR"] = str(env_dir)

    result = subprocess.run(
        ["cargo", "run", "--quiet", "--", "--init-db"],
        cwd=ROOT,
        env=env,
        capture_output=True,
        text=True,
    )

    if result.returncode != 0:
        click.echo(click.style(f"Init failed: {result.stderr}", fg="red"))
        raise click.ClickException("Failed to initialize database")

    # Parse output for schema version
    for line in result.stderr.split("\n"):
        if "Schema version:" in line:
            click.echo(f"  - {line.split(']')[-1].strip()}")
        if "Seeded" in line and "baseline cards" in line:
            click.echo(f"  - {line.split(']')[-1].strip()}")

    click.echo(f"  - Users directory: {users_dir}")

    click.echo(click.style(f"\nTest environment '{name}' ready!", fg="green"))
    click.echo(f"\nTo use with server:")
    click.echo(f"  DATA_DIR={env_dir} cargo run")


@cli.command("cleanup-test-env")
@click.argument("name")
@click.option(
    "--data-dir",
    "-d",
    type=click.Path(),
    default=None,
    help="Override data directory path.",
)
@click.option(
    "--yes",
    "-y",
    is_flag=True,
    help="Skip confirmation prompt.",
)
def cleanup_test_env(name: str, data_dir: str | None, yes: bool) -> None:
    """Remove a test environment completely.

    Deletes the entire test environment directory including:
    - app.db
    - All user databases
    - All generated content

    Examples:
        db-manager cleanup-test-env auth-tests
        db-manager cleanup-test-env auth-tests --yes
    """
    # Determine environment directory
    if data_dir:
        env_dir = Path(data_dir)
    else:
        env_dir = get_test_env_dir(name)

    if not env_dir.exists():
        click.echo(f"Environment '{name}' not found at {env_dir}")
        return

    if not yes:
        click.echo(f"This will delete the entire test environment at:")
        click.echo(f"  {env_dir}")
        if not click.confirm("Continue?"):
            click.echo("Aborted.")
            return

    shutil.rmtree(env_dir)
    click.echo(click.style(f"Test environment '{name}' removed.", fg="green"))


@cli.command("cleanup-test-users")
@click.option(
    "--prefix",
    "-p",
    type=str,
    default="_test_",
    help="User prefix to match (default: _test_).",
)
@click.option(
    "--data-dir",
    "-d",
    type=click.Path(),
    default=None,
    help="Specific data directory to clean (default: all).",
)
@click.option(
    "--yes",
    "-y",
    is_flag=True,
    help="Skip confirmation prompt.",
)
def cleanup_test_users(prefix: str, data_dir: str | None, yes: bool) -> None:
    """Remove all test users matching a prefix.

    Useful for cleaning up after E2E test crashes that leave orphaned users.

    Examples:
        db-manager cleanup-test-users                    # Remove all _test_* users
        db-manager cleanup-test-users --prefix _e2e_    # Remove _e2e_* users
        db-manager cleanup-test-users --data-dir data/test/auth  # Specific env
    """
    # Determine which app.db to check
    if data_dir:
        app_db = Path(data_dir) / "app.db"
        users_base = Path(data_dir) / "users"
    else:
        app_db = AUTH_DB
        users_base = USERS_DIR

    if not app_db.exists():
        click.echo(f"Database not found: {app_db}")
        return

    # Find matching users
    conn = sqlite3.connect(app_db)
    try:
        users = conn.execute(
            "SELECT username FROM users WHERE username LIKE ?",
            (f"{prefix}%",)
        ).fetchall()
    finally:
        conn.close()

    if not users:
        click.echo(f"No users found matching prefix '{prefix}'")
        return

    usernames = [u[0] for u in users]
    click.echo(f"Found {len(usernames)} users matching '{prefix}*':")
    for u in usernames[:10]:
        click.echo(f"  - {u}")
    if len(usernames) > 10:
        click.echo(f"  ... and {len(usernames) - 10} more")

    if not yes:
        if not click.confirm(f"Delete all {len(usernames)} users?"):
            click.echo("Aborted.")
            return

    # Delete users
    conn = sqlite3.connect(app_db)
    try:
        conn.execute(
            "DELETE FROM users WHERE username LIKE ?",
            (f"{prefix}%",)
        )
        conn.commit()
    finally:
        conn.close()

    # Delete user directories
    deleted_dirs = 0
    for username in usernames:
        user_dir = users_base / username
        if user_dir.exists():
            shutil.rmtree(user_dir)
            deleted_dirs += 1

    click.echo(click.style(
        f"Deleted {len(usernames)} users and {deleted_dirs} directories.",
        fg="green"
    ))


@cli.command("create-test-user")
@click.argument("username")
@click.option(
    "--password",
    "-p",
    type=str,
    default="test123",
    help="Password (default: test123).",
)
@click.option(
    "--scenario",
    "-s",
    type=str,
    default=None,
    help="Apply scenario preset after creation.",
)
@click.option(
    "--data-dir",
    "-d",
    type=click.Path(),
    default=None,
    help="Data directory for the test environment.",
)
def create_test_user(username: str, password: str, scenario: str | None, data_dir: str | None) -> None:
    """Create a test user with optional scenario in one command.

    Combines create-user, create-scenario, and use into a single operation.
    Designed for E2E test fixtures that need quick user setup.

    Examples:
        db-manager create-test-user _test_alice
        db-manager create-test-user _test_bob --scenario tier3_fresh
        db-manager create-test-user _test_carol --data-dir data/test/auth
    """
    # Override paths if data-dir specified (resolve to absolute)
    if data_dir:
        env_dir = Path(data_dir).resolve()
        app_db = env_dir / "app.db"
        users_base = env_dir / "users"
    else:
        app_db = AUTH_DB
        users_base = USERS_DIR

    if not app_db.exists():
        raise click.ClickException(f"Database not found: {app_db}. Run init-test-env first.")

    # Check if user exists
    conn = sqlite3.connect(app_db)
    try:
        exists = conn.execute(
            "SELECT COUNT(*) FROM users WHERE username = ?",
            (username,)
        ).fetchone()[0]
        if exists:
            click.echo(f"User '{username}' already exists, skipping creation")
        else:
            # Create user in auth database
            password_hash = hash_password_for_storage(password, username)
            now = datetime.now().isoformat()
            conn.execute(
                """INSERT INTO users (username, password_hash, created_at, is_guest, last_activity_at)
                   VALUES (?, ?, ?, 0, ?)""",
                (username, password_hash, now, now),
            )
            conn.commit()
            click.echo(f"Created user: {username}")
    finally:
        conn.close()

    # Create user directory and learning database using Rust CLI
    # This ensures schema matches exactly what the server expects
    user_dir = users_base / username
    user_db_path = user_dir / "learning.db"

    if not user_db_path.exists():
        import subprocess

        # Use Rust CLI to initialize learning.db with correct schema
        env = os.environ.copy()
        if data_dir:
            env["DATA_DIR"] = str(env_dir)

        result = subprocess.run(
            ["cargo", "run", "--release", "--", "--init-user-db", username],
            cwd=ROOT,
            env=env,
            capture_output=True,
            text=True,
        )
        if result.returncode != 0:
            raise click.ClickException(
                f"Failed to initialize learning.db: {result.stderr}"
            )
        click.echo(f"Created learning database: {user_db_path}")

    # Apply scenario if specified
    if scenario:
        if scenario not in SCENARIO_PRESETS:
            raise click.ClickException(
                f"Unknown scenario: {scenario}\n"
                f"Available: {', '.join(SCENARIO_PRESETS.keys())}"
            )

        desc, apply_fn = SCENARIO_PRESETS[scenario]
        conn = sqlite3.connect(user_db_path)
        try:
            apply_fn(conn, click.echo)
            click.echo(f"Applied scenario: {scenario}")
        finally:
            conn.close()

    click.echo(click.style(f"Test user '{username}' ready!", fg="green"))


def get_schema_info(db_path: Path) -> dict:
    """Get schema version and table counts from a database."""
    if not db_path.exists():
        return {"error": "Database not found"}

    conn = sqlite3.connect(db_path)
    try:
        result = {"version": 0, "tables": {}}

        # Get schema version
        try:
            row = conn.execute("SELECT MAX(version) FROM db_version").fetchone()
            result["version"] = row[0] if row and row[0] else 0
        except sqlite3.OperationalError:
            pass

        # Count card_definitions if it exists
        try:
            count = conn.execute("SELECT COUNT(*) FROM card_definitions").fetchone()[0]
            result["tables"]["card_definitions"] = count
        except sqlite3.OperationalError:
            result["tables"]["card_definitions"] = "MISSING"

        return result
    finally:
        conn.close()


@cli.command("list-test-envs")
def list_test_envs() -> None:
    """List all test environments.

    Shows test environments in data/test/ with their status.
    """
    test_dir = DATA_DIR / "test"
    if not test_dir.exists():
        click.echo("No test environments found.")
        click.echo(f"\nCreate one with: db-manager init-test-env <name>")
        return

    click.echo(click.style("=== Test Environments ===", bold=True))
    click.echo()

    for env_dir in sorted(test_dir.iterdir()):
        if not env_dir.is_dir():
            continue

        name = env_dir.name
        app_db = env_dir / "app.db"
        users_dir = env_dir / "users"

        if app_db.exists():
            result = get_schema_info(app_db)
            version = result.get("version", "?")
            cards = result.get("tables", {}).get("card_definitions", 0)
            status = click.style("OK", fg="green")
        else:
            version = "-"
            cards = 0
            status = click.style("NO DB", fg="red")

        # Count users
        user_count = len(list(users_dir.glob("*"))) if users_dir.exists() else 0

        click.echo(f"  {name:20} v{version}  {cards:3} cards  {user_count:3} users  [{status}]")


# =============================================================================
# User Role Management
# =============================================================================


@cli.command("set-role")
@click.argument("username")
@click.argument("role", type=click.Choice(["user", "admin"]))
@click.option(
    "--data-dir",
    "-d",
    type=click.Path(),
    default=None,
    help="Data directory (for test environments).",
)
def set_role(username: str, role: str, data_dir: str | None) -> None:
    """Set a user's role (user or admin).

    Example:
        db-manager set-role alice admin
        db-manager set-role bob user --data-dir data/test/e2e
    """
    app_db = get_app_db_path(Path(data_dir) if data_dir else None)

    if not app_db.exists():
        raise click.ClickException(f"Auth database not found: {app_db}")

    if not user_exists_in_env(username, Path(data_dir) if data_dir else None):
        raise click.ClickException(f"User not found: {username}")

    conn = sqlite3.connect(app_db)
    try:
        conn.execute(
            "UPDATE users SET role = ? WHERE username = ?",
            (role, username),
        )
        conn.commit()
        click.echo(f"Set {username} role to {role}")
    finally:
        conn.close()


# =============================================================================
# Group Management
# =============================================================================


@cli.command("create-group")
@click.argument("group_id")
@click.argument("name")
@click.option(
    "--description",
    "-desc",
    type=str,
    default=None,
    help="Optional group description.",
)
@click.option(
    "--data-dir",
    "-d",
    type=click.Path(),
    default=None,
    help="Data directory (for test environments).",
)
def create_group(group_id: str, name: str, description: str | None, data_dir: str | None) -> None:
    """Create a new user group.

    Example:
        db-manager create-group premium "Premium Users"
        db-manager create-group beta "Beta Testers" --description "Early access group"
    """
    app_db = get_app_db_path(Path(data_dir) if data_dir else None)

    if not app_db.exists():
        raise click.ClickException(f"Auth database not found: {app_db}")

    conn = sqlite3.connect(app_db)
    try:
        # Check if group already exists
        existing = conn.execute(
            "SELECT id FROM user_groups WHERE id = ?", (group_id,)
        ).fetchone()
        if existing:
            raise click.ClickException(f"Group already exists: {group_id}")

        now = datetime.now().isoformat()
        conn.execute(
            "INSERT INTO user_groups (id, name, description, created_at) VALUES (?, ?, ?, ?)",
            (group_id, name, description, now),
        )
        conn.commit()
        click.echo(f"Created group: {group_id} ({name})")
    finally:
        conn.close()


@cli.command("delete-group")
@click.argument("group_id")
@click.option(
    "--yes",
    "-y",
    is_flag=True,
    help="Skip confirmation prompt.",
)
@click.option(
    "--data-dir",
    "-d",
    type=click.Path(),
    default=None,
    help="Data directory (for test environments).",
)
def delete_group(group_id: str, yes: bool, data_dir: str | None) -> None:
    """Delete a user group and all memberships.

    Example:
        db-manager delete-group beta
        db-manager delete-group old-group --yes
    """
    app_db = get_app_db_path(Path(data_dir) if data_dir else None)

    if not app_db.exists():
        raise click.ClickException(f"Auth database not found: {app_db}")

    conn = sqlite3.connect(app_db)
    try:
        # Check if group exists
        existing = conn.execute(
            "SELECT name FROM user_groups WHERE id = ?", (group_id,)
        ).fetchone()
        if not existing:
            raise click.ClickException(f"Group not found: {group_id}")

        group_name = existing[0]

        # Count members
        member_count = conn.execute(
            "SELECT COUNT(*) FROM user_group_members WHERE group_id = ?", (group_id,)
        ).fetchone()[0]

        if not yes:
            click.confirm(
                f"Delete group '{group_name}' ({group_id}) with {member_count} members?",
                abort=True,
            )

        # Delete memberships first
        conn.execute("DELETE FROM user_group_members WHERE group_id = ?", (group_id,))
        # Delete pack permissions for this group
        conn.execute("DELETE FROM pack_permissions WHERE group_id = ?", (group_id,))
        # Delete the group
        conn.execute("DELETE FROM user_groups WHERE id = ?", (group_id,))
        conn.commit()

        click.echo(f"Deleted group: {group_id}")
    finally:
        conn.close()


@cli.command("add-to-group")
@click.argument("username")
@click.argument("group_id")
@click.option(
    "--data-dir",
    "-d",
    type=click.Path(),
    default=None,
    help="Data directory (for test environments).",
)
def add_to_group(username: str, group_id: str, data_dir: str | None) -> None:
    """Add a user to a group.

    Example:
        db-manager add-to-group alice premium
    """
    app_db = get_app_db_path(Path(data_dir) if data_dir else None)

    if not app_db.exists():
        raise click.ClickException(f"Auth database not found: {app_db}")

    conn = sqlite3.connect(app_db)
    try:
        # Get user ID
        user = conn.execute(
            "SELECT id FROM users WHERE username = ?", (username,)
        ).fetchone()
        if not user:
            raise click.ClickException(f"User not found: {username}")
        user_id = user[0]

        # Check group exists
        group = conn.execute(
            "SELECT name FROM user_groups WHERE id = ?", (group_id,)
        ).fetchone()
        if not group:
            raise click.ClickException(f"Group not found: {group_id}")

        # Check if already a member
        existing = conn.execute(
            "SELECT 1 FROM user_group_members WHERE group_id = ? AND user_id = ?",
            (group_id, user_id),
        ).fetchone()
        if existing:
            click.echo(f"{username} is already a member of {group_id}")
            return

        conn.execute(
            "INSERT INTO user_group_members (group_id, user_id) VALUES (?, ?)",
            (group_id, user_id),
        )
        conn.commit()
        click.echo(f"Added {username} to group {group_id}")
    finally:
        conn.close()


@cli.command("remove-from-group")
@click.argument("username")
@click.argument("group_id")
@click.option(
    "--data-dir",
    "-d",
    type=click.Path(),
    default=None,
    help="Data directory (for test environments).",
)
def remove_from_group(username: str, group_id: str, data_dir: str | None) -> None:
    """Remove a user from a group.

    Example:
        db-manager remove-from-group alice premium
    """
    app_db = get_app_db_path(Path(data_dir) if data_dir else None)

    if not app_db.exists():
        raise click.ClickException(f"Auth database not found: {app_db}")

    conn = sqlite3.connect(app_db)
    try:
        # Get user ID
        user = conn.execute(
            "SELECT id FROM users WHERE username = ?", (username,)
        ).fetchone()
        if not user:
            raise click.ClickException(f"User not found: {username}")
        user_id = user[0]

        # Check if member
        existing = conn.execute(
            "SELECT 1 FROM user_group_members WHERE group_id = ? AND user_id = ?",
            (group_id, user_id),
        ).fetchone()
        if not existing:
            click.echo(f"{username} is not a member of {group_id}")
            return

        conn.execute(
            "DELETE FROM user_group_members WHERE group_id = ? AND user_id = ?",
            (group_id, user_id),
        )
        conn.commit()
        click.echo(f"Removed {username} from group {group_id}")
    finally:
        conn.close()


@cli.command("get-group-count")
@click.option(
    "--data-dir",
    "-d",
    type=click.Path(),
    default=None,
    help="Data directory (for test environments).",
)
def get_group_count(data_dir: str | None) -> None:
    """Get the number of groups in the database.

    Outputs just the count number (useful for E2E test assertions).

    Example:
        db-manager get-group-count
        db-manager get-group-count --data-dir data/test/e2e
    """
    app_db = get_app_db_path(Path(data_dir) if data_dir else None)

    if not app_db.exists():
        click.echo("0")
        return

    conn = sqlite3.connect(app_db)
    try:
        count = conn.execute("SELECT COUNT(*) FROM user_groups").fetchone()[0]
        click.echo(str(count))
    finally:
        conn.close()


@cli.command("get-guest-count")
@click.option(
    "--data-dir",
    "-d",
    type=click.Path(),
    default=None,
    help="Data directory (for test environments).",
)
def get_guest_count(data_dir: str | None) -> None:
    """Get the number of guest users in the database.

    Outputs just the count number (useful for E2E test assertions).

    Example:
        db-manager get-guest-count
        db-manager get-guest-count --data-dir data/test/e2e
    """
    app_db = get_app_db_path(Path(data_dir) if data_dir else None)

    if not app_db.exists():
        click.echo("0")
        return

    conn = sqlite3.connect(app_db)
    try:
        count = conn.execute("SELECT COUNT(*) FROM users WHERE is_guest = 1").fetchone()[0]
        click.echo(str(count))
    finally:
        conn.close()


@cli.command("guest-exists")
@click.argument("guest_id")
@click.option(
    "--data-dir",
    "-d",
    type=click.Path(),
    default=None,
    help="Data directory (for test environments).",
)
def guest_exists_cmd(guest_id: str, data_dir: str | None) -> None:
    """Check if a specific guest user exists.

    Outputs 'true' or 'false' (useful for E2E test assertions).

    Example:
        db-manager guest-exists _guest_abc123
        db-manager guest-exists _guest_test --data-dir data/test/e2e
    """
    # Ensure username starts with _guest_ prefix
    username = guest_id if guest_id.startswith("_guest_") else f"_guest_{guest_id}"

    exists = user_exists_in_env(username, Path(data_dir) if data_dir else None)
    click.echo("true" if exists else "false")


@cli.command("create-expired-guest")
@click.argument("guest_id")
@click.option(
    "--data-dir",
    "-d",
    type=click.Path(),
    default=None,
    help="Data directory (for test environments).",
)
def create_expired_guest(guest_id: str, data_dir: str | None) -> None:
    """Create a guest user with an already-expired session.

    Used by E2E tests to verify guest cleanup functionality.
    The guest will have last_activity_at set 48 hours in the past.

    Example:
        db-manager create-expired-guest _guest_old
        db-manager create-expired-guest _guest_test --data-dir data/test/e2e
    """
    # Ensure username starts with _guest_ prefix
    username = guest_id if guest_id.startswith("_guest_") else f"_guest_{guest_id}"

    # Determine paths
    if data_dir:
        env_dir = Path(data_dir).resolve()
        app_db = env_dir / "app.db"
        users_base = env_dir / "users"
    else:
        app_db = AUTH_DB
        users_base = USERS_DIR

    if not app_db.exists():
        raise click.ClickException(f"Database not found: {app_db}. Run init-test-env first.")

    # Check if user already exists
    conn = sqlite3.connect(app_db)
    try:
        exists = conn.execute(
            "SELECT COUNT(*) FROM users WHERE username = ?",
            (username,)
        ).fetchone()[0]
        if exists:
            raise click.ClickException(f"Guest already exists: {username}")

        # Create guest user with expired timestamp (48 hours ago)
        password_hash = hash_password_for_storage("guest", username)
        now = datetime.now()
        expired_at = (now - timedelta(hours=48)).isoformat()

        conn.execute(
            """INSERT INTO users (username, password_hash, created_at, is_guest, last_activity_at)
               VALUES (?, ?, ?, 1, ?)""",
            (username, password_hash, expired_at, expired_at),
        )
        conn.commit()
        click.echo(f"Created expired guest: {username}")
    finally:
        conn.close()

    # Create user directory and learning database using Rust CLI
    user_dir = users_base / username
    user_db_path = user_dir / "learning.db"

    if not user_db_path.exists():
        import subprocess

        env = os.environ.copy()
        if data_dir:
            env["DATA_DIR"] = str(env_dir)

        result = subprocess.run(
            ["cargo", "run", "--release", "--", "--init-user-db", username],
            cwd=ROOT,
            env=env,
            capture_output=True,
            text=True,
        )
        if result.returncode != 0:
            # Non-fatal - guest can exist without learning.db for cleanup tests
            click.echo(f"Warning: Could not create learning.db: {result.stderr}")

    click.echo(click.style(f"Expired guest '{username}' ready for cleanup testing!", fg="green"))


@cli.command("list-groups")
@click.option(
    "--data-dir",
    "-d",
    type=click.Path(),
    default=None,
    help="Data directory (for test environments).",
)
def list_groups(data_dir: str | None) -> None:
    """List all groups and their members.

    Example:
        db-manager list-groups
    """
    app_db = get_app_db_path(Path(data_dir) if data_dir else None)

    if not app_db.exists():
        raise click.ClickException(f"Auth database not found: {app_db}")

    conn = sqlite3.connect(app_db)
    try:
        groups = conn.execute(
            "SELECT id, name, description FROM user_groups ORDER BY id"
        ).fetchall()

        if not groups:
            click.echo("No groups found.")
            return

        click.echo(click.style("=== Groups ===", bold=True))
        for group_id, name, desc in groups:
            # Get members
            members = conn.execute(
                """SELECT u.username FROM user_group_members gm
                   JOIN users u ON gm.user_id = u.id
                   WHERE gm.group_id = ?
                   ORDER BY u.username""",
                (group_id,),
            ).fetchall()

            member_str = ", ".join(m[0] for m in members) if members else "(empty)"
            click.echo(f"\n{click.style(name, bold=True)} ({group_id})")
            if desc:
                click.echo(f"  {desc}")
            click.echo(f"  Members: {member_str}")
    finally:
        conn.close()


# =============================================================================
# Pack Lesson Queries (for testing)
# =============================================================================


@cli.command("get-pack-lesson-counts")
@click.argument("pack_id")
@click.option(
    "--data-dir",
    "-d",
    type=click.Path(),
    default=None,
    help="Data directory (for test environments).",
)
@click.option(
    "--json",
    "as_json",
    is_flag=True,
    help="Output as JSON.",
)
def get_pack_lesson_counts(pack_id: str, data_dir: str | None, as_json: bool) -> None:
    """Get card counts per lesson for a pack.

    Queries card_definitions to show how many cards each lesson has.
    Used by integration tests to verify lesson filtering.

    Outputs lesson:count pairs. Use --json for machine-readable output.

    Example:
        db-manager get-pack-lesson-counts my_vocab_pack
        db-manager get-pack-lesson-counts my_vocab_pack --json
        db-manager get-pack-lesson-counts test_pack --data-dir data/test/e2e
    """
    import json

    app_db = get_app_db_path(Path(data_dir) if data_dir else None)

    if not app_db.exists():
        raise click.ClickException(f"Auth database not found: {app_db}")

    conn = sqlite3.connect(app_db)
    try:
        # Query card counts by lesson
        rows = conn.execute(
            """SELECT lesson, COUNT(*) as count
               FROM card_definitions
               WHERE pack_id = ?
               GROUP BY lesson
               ORDER BY lesson""",
            (pack_id,),
        ).fetchall()

        if not rows:
            if as_json:
                click.echo("{}")
            else:
                click.echo(f"No cards found for pack: {pack_id}")
            return

        # Build result dict (convert None lesson to "null" string for JSON)
        result = {}
        for lesson, count in rows:
            key = lesson if lesson is not None else "null"
            result[key] = count

        if as_json:
            click.echo(json.dumps(result))
        else:
            click.echo(f"Pack: {pack_id}")
            total = 0
            for lesson, count in rows:
                lesson_str = f"Lesson {lesson}" if lesson is not None else "No lesson"
                click.echo(f"  {lesson_str}: {count} cards")
                total += count
            click.echo(f"  Total: {total} cards")
    finally:
        conn.close()


# ==================== Validation Feedback ====================


@cli.command("list-feedback")
@click.option("--user", "-u", type=str, default=None, help="Filter by username.")
@click.option("--pending", is_flag=True, help="Show only unreviewed feedback.")
@click.option(
    "--data-dir",
    "-d",
    type=click.Path(),
    default=None,
    help="Data directory (for test environments).",
)
@click.option("--json", "as_json", is_flag=True, help="Output as JSON.")
def list_feedback(
    user: str | None, pending: bool, data_dir: str | None, as_json: bool
) -> None:
    """List validation feedback from user overrides.

    Shows feedback submitted when users override answer validation rulings
    during study. Useful for reviewing and improving validation rules.

    Examples:
        db-manager list-feedback
        db-manager list-feedback --user alice
        db-manager list-feedback --pending
        db-manager list-feedback --json
    """
    import json

    app_db = get_app_db_path(Path(data_dir) if data_dir else None)

    if not app_db.exists():
        raise click.ClickException(f"Auth database not found: {app_db}")

    conn = sqlite3.connect(app_db)
    try:
        # Check if table exists (requires schema v10+)
        table_exists = conn.execute(
            "SELECT COUNT(*) FROM sqlite_master WHERE type='table' AND name='validation_suggestions'"
        ).fetchone()[0]

        if not table_exists:
            if as_json:
                click.echo("[]")
            else:
                click.echo("No validation_suggestions table (requires schema v10+)")
            return

        # Build query with optional filters
        query = """SELECT id, username, card_front, expected_answer, user_answer,
                          suggested_answer, user_quality, created_at, reviewed_at, pack_id
                   FROM validation_suggestions"""
        conditions = []
        params: list = []

        if user:
            conditions.append("username = ?")
            params.append(user)

        if pending:
            conditions.append("reviewed_at IS NULL")

        if conditions:
            query += " WHERE " + " AND ".join(conditions)

        query += " ORDER BY created_at DESC"

        rows = conn.execute(query, params).fetchall()

        if as_json:
            results = []
            for row in rows:
                results.append(
                    {
                        "id": row[0],
                        "username": row[1],
                        "card_front": row[2],
                        "expected_answer": row[3],
                        "user_answer": row[4],
                        "suggested_answer": row[5],
                        "quality": row[6],
                        "created_at": row[7],
                        "reviewed_at": row[8],
                        "pack_id": row[9],
                    }
                )
            click.echo(json.dumps(results, indent=2))
        else:
            if not rows:
                click.echo("No feedback entries found.")
                return

            click.echo(f"Found {len(rows)} feedback entries:\n")
            for row in rows:
                status = "pending" if row[8] is None else f"reviewed ({row[8]})"
                quality_map = {0: "Wrong", 2: "Hard", 4: "Correct", 5: "Easy"}
                quality_str = quality_map.get(row[6], str(row[6]))
                click.echo(f"[{row[0]}] {row[1]} - {row[7]} ({status})")
                click.echo(f"  Card: {row[2]}")
                click.echo(f"  Expected: {row[3]}")
                click.echo(f"  User answered: {row[4]}")
                click.echo(f"  Suggested: {row[5]}")
                click.echo(f"  Rating: {quality_str}")
                if row[9]:
                    click.echo(f"  Pack: {row[9]}")
                click.echo()
    finally:
        conn.close()


@cli.command("clear-feedback")
@click.option("--reviewed", is_flag=True, help="Clear only reviewed feedback.")
@click.option("--all", "clear_all", is_flag=True, help="Clear all feedback.")
@click.option(
    "--id", "feedback_id", type=int, default=None, help="Clear specific feedback by ID."
)
@click.option(
    "--data-dir",
    "-d",
    type=click.Path(),
    default=None,
    help="Data directory (for test environments).",
)
def clear_feedback(
    reviewed: bool, clear_all: bool, feedback_id: int | None, data_dir: str | None
) -> None:
    """Clear validation feedback entries.

    Requires one of: --reviewed, --all, or --id

    Examples:
        db-manager clear-feedback --reviewed    # Clear reviewed entries
        db-manager clear-feedback --all         # Clear all entries
        db-manager clear-feedback --id 42       # Clear specific entry
    """
    # Validate options
    options_count = sum([reviewed, clear_all, feedback_id is not None])
    if options_count == 0:
        raise click.ClickException("Specify one of: --reviewed, --all, or --id")
    if options_count > 1:
        raise click.ClickException("Specify only one of: --reviewed, --all, or --id")

    app_db = get_app_db_path(Path(data_dir) if data_dir else None)

    if not app_db.exists():
        raise click.ClickException(f"Auth database not found: {app_db}")

    conn = sqlite3.connect(app_db)
    try:
        # Check if table exists
        table_exists = conn.execute(
            "SELECT COUNT(*) FROM sqlite_master WHERE type='table' AND name='validation_suggestions'"
        ).fetchone()[0]

        if not table_exists:
            click.echo("No validation_suggestions table (requires schema v10+)")
            return

        # Build delete query
        if feedback_id is not None:
            # Check if entry exists
            count = conn.execute(
                "SELECT COUNT(*) FROM validation_suggestions WHERE id = ?",
                (feedback_id,),
            ).fetchone()[0]
            if count == 0:
                raise click.ClickException(f"Feedback entry not found: {feedback_id}")

            if not click.confirm(f"Delete feedback entry {feedback_id}?"):
                click.echo("Aborted.")
                return

            conn.execute(
                "DELETE FROM validation_suggestions WHERE id = ?", (feedback_id,)
            )
            conn.commit()
            click.echo(click.style(f"Deleted feedback entry {feedback_id}", fg="green"))

        elif reviewed:
            count = conn.execute(
                "SELECT COUNT(*) FROM validation_suggestions WHERE reviewed_at IS NOT NULL"
            ).fetchone()[0]

            if count == 0:
                click.echo("No reviewed feedback entries to delete.")
                return

            if not click.confirm(f"Delete {count} reviewed feedback entries?"):
                click.echo("Aborted.")
                return

            conn.execute(
                "DELETE FROM validation_suggestions WHERE reviewed_at IS NOT NULL"
            )
            conn.commit()
            click.echo(
                click.style(f"Deleted {count} reviewed feedback entries", fg="green")
            )

        elif clear_all:
            count = conn.execute(
                "SELECT COUNT(*) FROM validation_suggestions"
            ).fetchone()[0]

            if count == 0:
                click.echo("No feedback entries to delete.")
                return

            if not click.confirm(f"Delete ALL {count} feedback entries?"):
                click.echo("Aborted.")
                return

            conn.execute("DELETE FROM validation_suggestions")
            conn.commit()
            click.echo(click.style(f"Deleted all {count} feedback entries", fg="green"))
    finally:
        conn.close()
