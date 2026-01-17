# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Project Overview

Korean Hangul learning app using spaced repetition (FSRS algorithm). Multi-user Rust web application with per-user SQLite databases. Tech stack: Axum, rusqlite, Askama templates, HTMX 2.x, Tailwind CSS v4.

## Build Commands

```bash
# Run development server
cargo run

# Hot reload development
cargo watch -x run

# Run unit tests
cargo test

# Run single test
cargo test test_name

# Lint
cargo clippy

# Build release
cargo build --release

# Enable profiling (logs handler timing)
cargo run --features profiling

# Cross-compile for Raspberry Pi
cross build --release --target aarch64-unknown-linux-gnu
```

### Integration Tests (Python)

```bash
cd tests/integration
uv run pytest               # Spawns isolated server automatically (port 3100)
uv run pytest -v            # Verbose output
uv run pytest -k auth       # Filter by name
uv run pytest tests/test_offline_sync.py  # Run specific test file
```

The test suite uses a pytest fixture (`test_server`) that automatically:
1. Creates an ephemeral test data directory
2. Spawns an isolated server on port 3100
3. Cleans up after tests complete

### E2E Tests (Playwright)

```bash
cd tests/e2e
npm install && npx playwright install  # First time setup
npm test                               # Run all 8 test projects
npm run test:headed                    # With visible browser
npm run test:ui                        # Interactive UI
```

See `doc/08_testing.md` for full testing guide.

### Python Tools (py_scripts/)

```bash
cd py_scripts
uv sync                     # Install dependencies
uv run pytest               # Run tests
uv run ruff check .         # Lint
uv run ruff format .        # Format

# Audio scraper
uv run kr-scraper lesson1
uv run kr-scraper segment

# Database management
uv run db-manager list-users
uv run db-manager create-user alice --password secret
uv run db-manager info --user alice
```

### Docker

```bash
docker compose up -d                              # Start app
docker compose run --rm py-tools kr-scraper lesson1  # Run Python tools
```

## Architecture

### Dual-Database Design

- **app.db** (`data/app.db`): Shared database for users, sessions, card definitions, content packs, permissions
- **learning.db** (`data/users/<username>/learning.db`): Per-user SRS progress, reviews, settings

Cross-database queries use `ATTACH DATABASE` to join card definitions with user progress.

### Key Source Modules

| Path | Purpose |
|------|---------|
| `src/main.rs` | Server entry point, routes |
| `src/state.rs` | AppState (shared auth DB, paths) |
| `src/auth/` | Authentication (middleware, Argon2 hashing, sessions) |
| `src/db/` | User database layer (cards, reviews, stats, tiers) |
| `src/content/` | Pack discovery, card/audio loading, generators |
| `src/srs/` | FSRS and SM-2 spaced repetition algorithms |
| `src/handlers/` | HTTP handlers organized by feature |
| `src/validation.rs` | Answer validation with romanization variants |

### Content Pack System

Modular content packs in `data/content/packs/`. Each pack has a `pack.json` manifest. Types:
- **cards**: Flashcard definitions loaded into `card_definitions` table
- **audio**: Pronunciation audio files with fallback resolution
- **generator**: Scripts that produce content (e.g., web scrapers)

Pack discovery scans directories on settings page load. Cards are deduplicated by `(front, main_answer, card_type, tier, is_reverse)`.

### Template/Frontend Pattern

- Askama templates in `templates/` with compile-time type checking
- HTMX for interactivity without JavaScript
- Tailwind CSS built at compile time via `build.rs`
- Static assets hashed for cache busting

### Authentication Flow

1. Client hashes `password + username` with SHA-256 (never sends plaintext)
2. Server applies Argon2 to the SHA-256 hash (defense-in-depth)
3. Sessions stored in HTTP-only cookies (7-day expiry)
4. All routes except `/login`, `/register` require auth
5. Each request opens user's `learning.db` and attaches `app.db` for cross-DB queries

See `doc/04_authentication.md` for full auth system documentation.

### SRS Algorithms

- **FSRS** (default): Modern algorithm tracking stability and difficulty per card
- **SM-2**: Classic SuperMemo fallback
- Cards progress through learning steps (0-3) before graduating

## Database Schema Locations

- app.db schema: `src/auth/db.rs` (version 9)
- learning.db schema: `src/db/schema.rs` (version 5)
- Full documentation: `doc/05_database.md`

## Documentation

**Core Learning:**
- `doc/01_learning_fsa.md` - Learning mode state machine, learning steps
- `doc/02_card_selection.md` - Card selection algorithm, weights, reinforcement
- `doc/03_answer_validation.md` - Answer validation, typo tolerance

**System:**
- `doc/04_authentication.md` - Authentication, sessions, permissions
- `doc/05_database.md` - Complete database schema
- `doc/06_packs.md` - Content pack system

**Reference:**
- `doc/07_endpoints.md` - API endpoint reference (67 endpoints)
- `doc/08_testing.md` - Testing guide (unit, integration, E2E)

**Operations:**
- `doc/09_profiling.md` - Performance profiling
- `doc/10_scripts.md` - Deployment scripts
- `doc/11_responsiveness_guidance.md` - Mobile UI patterns
