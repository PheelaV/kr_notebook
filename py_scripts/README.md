# kr_notebook Python Scripts

Python tools for kr_notebook development and content management.

## Installation

```bash
cd py_scripts
uv sync
```

## Tools

### kr-scraper

Content generation tools for kr_notebook.

```bash
uv run kr-scraper --help
```

#### Audio Scraping

```bash
uv run kr-scraper lesson1    # Scrape lesson 1 audio
uv run kr-scraper lesson2    # Scrape lesson 2 audio
uv run kr-scraper lesson3    # Scrape lesson 3 audio
```

#### Vocabulary to Cards

Convert vocabulary JSON to flashcard format:

```bash
# From a single vocabulary.json file
uv run kr-scraper vocabulary path/to/vocabulary.json

# From a directory of per-lesson files (lesson_01.json, lesson_02.json, etc.)
uv run kr-scraper vocabulary path/to/vocabulary/

# Options
uv run kr-scraper vocabulary path/to/vocabulary.json --tier 5      # Set card tier
uv run kr-scraper vocabulary path/to/vocabulary.json --no-reverse  # Skip reverse cards
uv run kr-scraper vocabulary path/to/vocabulary.json -o cards.json # Custom output path
```

When using a directory, files must be named `lesson_*.json` (e.g., `lesson_01.json`). Lesson numbers are auto-populated from filenames if not present in the JSON.

### db-manager

Database management for development and testing. Supports the multi-user architecture:
- **Auth DB**: `data/app.db` (users, sessions, app_settings)
- **User DBs**: `data/users/{username}/learning.db` (cards, progress, settings)

#### User Management

```bash
# List all users
uv run db-manager list-users

# Create a new user (with learning database seeded with 80 baseline cards)
uv run db-manager create-user alice --password secret123

# Delete a user and all their data
uv run db-manager delete-user alice --yes
```

#### Database Info

```bash
# Overview of all databases
uv run db-manager list

# Detailed info for a specific user
uv run db-manager info --user alice
```

Example output:
```
=== User Info: alice ===

Cards by Tier:
  Tier 1: 28/30 learned (93%), 0 new
  Tier 2: 14/14 learned (100%), 0 new
  Tier 3: 13/18 learned (72%), 0 new
  Tier 4: 0/18 learned (0%), 0 new

Settings:
  max_unlocked_tier: 3
  use_fsrs: true
```

#### Backups

```bash
# Create a timestamped backup
uv run db-manager backup --user alice
# Creates: data/backups/alice/20260104_160317.db
```

#### Test Scenarios

Scenarios let you quickly switch a user's database to predefined states for testing.

```bash
# List available presets
uv run db-manager create-scenario --list

# Available presets:
#   tier1_new       - Fresh start, tier 1 only, no reviews
#   tier3_fresh     - Tiers 1-2 graduated, tier 3 unlocked but new
#   tier3_unlock    - Tier 3 at 80% (about to unlock tier 4)
#   all_graduated   - All tiers unlocked and graduated

# Create a scenario for a user
uv run db-manager create-scenario --user alice tier3_fresh

# Switch user to that scenario (auto-backs up current state)
uv run db-manager use --user alice tier3_fresh

# Restore original database
uv run db-manager use --user alice original
```

#### Typical Testing Workflow

```bash
# 1. Create a test user
uv run db-manager create-user testuser --password test

# 2. Create scenarios you need
uv run db-manager create-scenario --user testuser tier3_fresh
uv run db-manager create-scenario --user testuser all_graduated

# 3. Switch between scenarios as needed
uv run db-manager use --user testuser tier3_fresh
# ... test tier 3 features ...

uv run db-manager use --user testuser all_graduated
# ... test graduated state ...

# 4. Clean up when done
uv run db-manager delete-user testuser --yes
```

## Directory Structure

```
py_scripts/
├── src/
│   ├── kr_scraper/      # Audio scraper modules
│   │   ├── __init__.py
│   │   ├── lesson1.py
│   │   ├── lesson2.py
│   │   └── lesson3.py
│   └── db_manager/      # Database management
│       ├── __init__.py
│       ├── cli.py       # CLI commands
│       └── fixtures.py  # Baseline card data
├── pyproject.toml
└── README.md
```

## Development

```bash
# Run tests
uv run pytest

# Lint
uv run ruff check .

# Format
uv run ruff format .
```
