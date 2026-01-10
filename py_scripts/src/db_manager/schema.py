"""Database schema reference.

This module documents the auth database schema (v0→v9) for reference purposes.
Actual database initialization is done by the Rust server using --init-db flag.

To create a test environment, use:
    db-manager init-test-env <name>

Which internally calls:
    DATA_DIR=<path> cargo run -- --init-db

This ensures the schema is always in sync with the Rust server's expectations.

Schema versions:
- v1: Base tables (users, sessions, app_settings)
- v2: Guest user support (is_guest, last_activity_at)
- v3: Content pack system (content_packs, card_definitions)
- v4: User roles and groups (user_groups, pack_permissions)
- v5: Direct user pack permissions (pack_user_permissions)
- v6: External pack paths (registered_pack_paths)
- v7: Lesson-based progression (lesson column, pack_ui_metadata)
- v8: Global pack control (is_enabled column)
- v9: Baseline pack registration and public permissions

See src/auth/db.rs for the authoritative schema implementation.
"""

# Schema is managed by Rust server. This file is for documentation only.
