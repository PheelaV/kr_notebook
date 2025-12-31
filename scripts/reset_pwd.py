# /// script
# requires-python = ">=3.12"
# dependencies = [
#     "argon2-cffi>=25.1.0",
# ]
# ///
"""Reset password for a user in the auth database.

Usage:
    uv run scripts/reset_pwd.py <username> <new_password>
    uv run scripts/reset_pwd.py admin mysecretpassword
"""

import hashlib
import sqlite3
import sys
from pathlib import Path

from argon2 import PasswordHasher


def reset_password(username: str, password: str, db_path: Path) -> None:
    """Reset password using the same flow as the Rust server."""
    # Step 1: Client-side hash (SHA-256 of "password:username")
    # This matches what the browser JavaScript does
    client_input = f"{password}:{username}"
    client_hash = hashlib.sha256(client_input.encode()).hexdigest()

    # Step 2: Server-side hash (Argon2 with default settings)
    ph = PasswordHasher()
    password_hash = ph.hash(client_hash)

    # Step 3: Update database
    conn = sqlite3.connect(db_path)
    cursor = conn.execute(
        "UPDATE users SET password_hash = ? WHERE username = ?",
        (password_hash, username),
    )
    if cursor.rowcount == 0:
        print(f"Error: User '{username}' not found")
        sys.exit(1)
    conn.commit()
    conn.close()

    print(f"Password reset for user '{username}'")


def main() -> None:
    if len(sys.argv) != 3:
        print("Usage: uv run scripts/reset_pwd.py <username> <new_password>")
        sys.exit(1)

    username = sys.argv[1]
    password = sys.argv[2]

    # Find the auth database
    script_dir = Path(__file__).parent
    project_root = script_dir.parent
    db_path = project_root / "data" / "app.db"

    if not db_path.exists():
        print(f"Error: Auth database not found at {db_path}")
        sys.exit(1)

    reset_password(username, password, db_path)


if __name__ == "__main__":
    main()
