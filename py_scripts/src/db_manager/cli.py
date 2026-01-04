"""Command-line interface for database management.

Updated for multi-user architecture:
- Auth DB: data/app.db (users, sessions, app_settings)
- User DBs: data/users/{username}/learning.db (per-user cards, progress, settings)
"""

import os
import secrets
import shutil
import sqlite3
from datetime import datetime
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


def get_user_db_path(username: str) -> Path:
    """Get the learning database path for a user."""
    return USERS_DIR / username / "learning.db"


def get_user_scenario_dir(username: str) -> Path:
    """Get the scenario directory for a user."""
    return SCENARIOS_DIR / username


def user_exists(username: str) -> bool:
    """Check if a user exists in the auth database."""
    if not AUTH_DB.exists():
        return False
    conn = sqlite3.connect(AUTH_DB)
    try:
        count = conn.execute(
            "SELECT COUNT(*) FROM users WHERE username = ?", (username,)
        ).fetchone()[0]
        return count > 0
    finally:
        conn.close()


def list_all_users() -> list[tuple[str, bool, str | None]]:
    """List all users: (username, is_guest, last_login_at)."""
    if not AUTH_DB.exists():
        return []
    conn = sqlite3.connect(AUTH_DB)
    try:
        rows = conn.execute(
            "SELECT username, COALESCE(is_guest, 0), last_login_at FROM users ORDER BY username"
        ).fetchall()
        return [(r[0], bool(r[1]), r[2]) for r in rows]
    finally:
        conn.close()


def hash_password_argon2(password: str) -> str:
    """Hash password using argon2id (compatible with Rust argon2 crate)."""
    try:
        from argon2 import PasswordHasher
        ph = PasswordHasher()
        return ph.hash(password)
    except ImportError:
        # Fallback: use a simple bcrypt-style placeholder
        # This won't work for actual login but is fine for test scenarios
        import hashlib
        click.echo(click.style("Warning: argon2-cffi not installed, using simple hash", fg="yellow"))
        return f"$test${hashlib.sha256(password.encode()).hexdigest()}"


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
    password_hash = hash_password_argon2(password)
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

    # Create and seed learning database
    user_db_path = get_user_db_path(username)
    init_learning_db(user_db_path)
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
def delete_user(username: str, yes: bool) -> None:
    """Delete a user and all their data.

    Removes:
    - Entry from data/app.db
    - Directory data/users/{username}/ (including learning.db)
    - Any scenarios in data/scenarios/{username}/

    Example:
        db-manager delete-user alice
        db-manager delete-user bob --yes
    """
    if not user_exists(username):
        raise click.ClickException(f"User not found: {username}")

    if not yes:
        click.echo(f"This will delete user '{username}' and ALL their data:")
        click.echo(f"  - Auth entry in {AUTH_DB}")
        click.echo(f"  - User directory {USERS_DIR / username}")
        click.echo(f"  - Scenarios in {SCENARIOS_DIR / username}")
        if not click.confirm("Continue?"):
            click.echo("Aborted.")
            return

    # Delete from auth database
    conn = sqlite3.connect(AUTH_DB)
    try:
        conn.execute("DELETE FROM users WHERE username = ?", (username,))
        conn.commit()
        click.echo(f"Removed from auth database: {username}")
    finally:
        conn.close()

    # Delete user directory
    user_dir = USERS_DIR / username
    if user_dir.exists():
        shutil.rmtree(user_dir)
        click.echo(f"Deleted user directory: {user_dir}")

    # Delete scenarios
    scenario_dir = get_user_scenario_dir(username)
    if scenario_dir.exists():
        shutil.rmtree(scenario_dir)
        click.echo(f"Deleted scenarios: {scenario_dir}")

    click.echo(click.style(f"User '{username}' deleted.", fg="green"))


@cli.command("list-users")
def list_users_cmd() -> None:
    """List all users in the auth database.

    Shows username, type (regular/guest), and last login time.
    """
    ensure_dirs()

    if not AUTH_DB.exists():
        click.echo("No auth database found. No users exist yet.")
        return

    users = list_all_users()
    if not users:
        click.echo("No users found.")
        return

    click.echo(click.style("=== Users ===", bold=True))
    click.echo()

    for username, is_guest, last_login in users:
        user_type = click.style("guest", fg="yellow") if is_guest else "user"
        login_str = last_login[:19] if last_login else "never"
        user_db = get_user_db_path(username)
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
    echo("Setting all cards to graduated...")
    conn.execute(f"UPDATE cards SET {GRADUATED_CARD_SQL}")
    conn.execute("INSERT OR REPLACE INTO settings (key, value) VALUES ('max_unlocked_tier', '4')")
    conn.execute("INSERT OR REPLACE INTO settings (key, value) VALUES ('all_tiers_unlocked', 'true')")
    conn.commit()


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
def create_scenario(preset: str | None, user: str | None, list_presets: bool) -> None:
    """Create a test scenario from a preset.

    Creates a modified copy of the user's learning database for testing.
    Scenarios are stored in data/scenarios/{username}/{preset}.db

    Examples:
        db-manager create-scenario --list
        db-manager create-scenario --user alice tier3_fresh
        db-manager create-scenario --user bob tier1_new
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

    if not user_exists(user):
        raise click.ClickException(f"User not found: {user}")

    # Source: user's current learning database or golden reference
    source_db = get_user_db_path(user)
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
    scenario_dir = get_user_scenario_dir(user)
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
        click.echo(f"To use: db-manager use --user {user} {preset}")
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
def use_scenario(name: str, user: str) -> None:
    """Switch a user's database to a scenario or back to production.

    Copies the scenario database over the user's learning.db.
    Creates a backup of the current database first.

    Examples:
        db-manager use --user alice tier3_fresh   # Use scenario
        db-manager use --user alice production    # Restore original
    """
    if not user_exists(user):
        raise click.ClickException(f"User not found: {user}")

    user_db = get_user_db_path(user)
    scenario_dir = get_user_scenario_dir(user)
    backup_dir = BACKUPS_DIR / user
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


def init_learning_db(db_path: Path) -> None:
    """Initialize a learning database with schema and seed cards."""
    conn = sqlite3.connect(db_path)
    try:
        # Create schema
        conn.executescript("""
            CREATE TABLE IF NOT EXISTS cards (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                front TEXT NOT NULL,
                main_answer TEXT NOT NULL,
                description TEXT,
                card_type TEXT NOT NULL,
                tier INTEGER NOT NULL,
                audio_hint TEXT,
                is_reverse INTEGER DEFAULT 0,
                ease_factor REAL DEFAULT 2.5,
                interval_days INTEGER DEFAULT 0,
                repetitions INTEGER DEFAULT 0,
                next_review TEXT DEFAULT (datetime('now')),
                total_reviews INTEGER DEFAULT 0,
                correct_reviews INTEGER DEFAULT 0,
                learning_step INTEGER DEFAULT 0,
                fsrs_stability REAL,
                fsrs_difficulty REAL,
                fsrs_state TEXT DEFAULT 'New',
                pack_id TEXT
            );

            CREATE TABLE IF NOT EXISTS review_logs (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                card_id INTEGER NOT NULL,
                quality INTEGER NOT NULL,
                reviewed_at TEXT NOT NULL,
                is_correct INTEGER,
                study_mode TEXT,
                direction TEXT,
                response_time_ms INTEGER,
                hints_used INTEGER DEFAULT 0,
                FOREIGN KEY (card_id) REFERENCES cards(id) ON DELETE CASCADE
            );

            CREATE TABLE IF NOT EXISTS settings (
                key TEXT PRIMARY KEY,
                value TEXT
            );

            CREATE TABLE IF NOT EXISTS confusions (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                card_id INTEGER NOT NULL,
                wrong_answer TEXT NOT NULL,
                count INTEGER DEFAULT 1,
                last_confused_at TEXT NOT NULL,
                FOREIGN KEY (card_id) REFERENCES cards(id) ON DELETE CASCADE
            );

            CREATE TABLE IF NOT EXISTS character_stats (
                character TEXT PRIMARY KEY,
                character_type TEXT NOT NULL,
                total_attempts INTEGER DEFAULT 0,
                total_correct INTEGER DEFAULT 0,
                attempts_7d INTEGER DEFAULT 0,
                correct_7d INTEGER DEFAULT 0,
                attempts_1d INTEGER DEFAULT 0,
                correct_1d INTEGER DEFAULT 0,
                last_attempt_at TEXT
            );

            CREATE TABLE IF NOT EXISTS tier_graduation_backups (
                tier INTEGER PRIMARY KEY,
                backup_data TEXT NOT NULL,
                created_at TEXT NOT NULL
            );

            CREATE TABLE IF NOT EXISTS enabled_packs (
                pack_id TEXT PRIMARY KEY,
                enabled_at TEXT,
                cards_created INTEGER DEFAULT 0,
                config TEXT
            );

            CREATE INDEX IF NOT EXISTS idx_cards_next_review ON cards(next_review);
            CREATE INDEX IF NOT EXISTS idx_cards_tier ON cards(tier);
            CREATE INDEX IF NOT EXISTS idx_review_logs_card_id ON review_logs(card_id);
            CREATE INDEX IF NOT EXISTS idx_review_logs_reviewed_at ON review_logs(reviewed_at);
            CREATE INDEX IF NOT EXISTS idx_confusions_card_id ON confusions(card_id);
            CREATE INDEX IF NOT EXISTS idx_character_stats_type ON character_stats(character_type);
        """)

        # Seed with baseline cards (imported from fixtures)
        from .fixtures import BASELINE_CARDS
        for card in BASELINE_CARDS:
            conn.execute(
                """INSERT INTO cards (front, main_answer, description, card_type, tier, is_reverse)
                   VALUES (?, ?, ?, ?, ?, ?)""",
                (card["front"], card["main_answer"], card.get("description"),
                 card["card_type"], card["tier"], card.get("is_reverse", 0))
            )

        # Default settings
        conn.execute("INSERT OR IGNORE INTO settings (key, value) VALUES ('max_unlocked_tier', '1')")

        conn.commit()
    finally:
        conn.close()
