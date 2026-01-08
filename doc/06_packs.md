# Content Pack System

This document describes the modular content pack system used by kr_notebook.

## Overview

Content packs are self-contained bundles of learning content that can be installed and enabled by users. The system supports three types of content:

- **Cards**: Flashcard definitions for SRS study
- **Audio**: Pronunciation audio files
- **Generators**: Scripts that produce content (e.g., web scrapers)

### Architecture

```
┌─────────────────────────────────────────────────────────────────┐
│                         Pack Discovery                           │
│  discover_packs() scans directories for pack.json manifests      │
└─────────────────────────────────────────────────────────────────┘
                              │
                              ▼
┌─────────────────────────────────────────────────────────────────┐
│                      PackLocation[]                              │
│  manifest: PackManifest, path: PathBuf, scope: Shared|User       │
└─────────────────────────────────────────────────────────────────┘
                              │
              ┌───────────────┼───────────────┐
              ▼               ▼               ▼
        ┌──────────┐   ┌──────────┐   ┌──────────┐
        │  Cards   │   │  Audio   │   │Generator │
        │  Pack    │   │  Pack    │   │  Pack    │
        └──────────┘   └──────────┘   └──────────┘
              │               │               │
              ▼               ▼               ▼
┌─────────────────────┐ ┌───────────┐ ┌──────────────────┐
│ card_definitions    │ │ Audio     │ │ Runs scraper     │
│ (app.db)            │ │ resolution│ │ Output → files   │
│                     │ │ fallback  │ │                  │
│ enabled_packs       │ └───────────┘ └──────────────────┘
│ (learning.db)       │
└─────────────────────┘
```

### Two-Database Design

| Database | Location | Purpose |
|----------|----------|---------|
| `app.db` | `data/app.db` | Shared: users, sessions, content_packs, card_definitions |
| `learning.db` | `data/users/{username}/learning.db` | Per-user: card_progress, enabled_packs, settings |

This separation allows:
- Card definitions to be shared (no duplication when multiple users enable same pack)
- Per-user enablement and progress tracking
- Efficient storage and queries

---

## Pack Manifest (pack.json)

Every pack requires a `pack.json` manifest in its root directory.

### Required Fields

| Field | Type | Description |
|-------|------|-------------|
| `id` | string | Unique identifier (e.g., `my-vocabulary`) |
| `name` | string | Human-readable name |
| `type` | string | Pack type: `cards`, `audio`, or `generator` |

### Optional Fields

| Field | Type | Description |
|-------|------|-------------|
| `version` | string | Semver version (e.g., `1.0.0`) |
| `description` | string | Brief description |
| `provides` | string[] | Content types this pack provides (e.g., `["vocabulary"]`) |
| `source` | object | Attribution and source info |

### The `provides` Field

The `provides` field declares what content types a pack offers. This enables dynamic UI features based on available content rather than hardcoded pack IDs.

**Current content types:**
- `vocabulary` - Enables the vocabulary library page (`/library/vocabulary`)

**How it works:**
1. Pack discovery scans for packs with matching `provides` values
2. UI features are shown/hidden based on available content types
3. Users only see features for content they can actually access

**Example:** A vocabulary pack declares `"provides": ["vocabulary"]`. The vocabulary library link only appears if at least one installed pack provides vocabulary content.

### Type-Specific Configuration

#### Cards Pack

```json
{
  "id": "my-vocabulary",
  "name": "My Vocabulary Pack",
  "version": "1.0.0",
  "type": "cards",
  "description": "Custom vocabulary flashcards",
  "provides": ["vocabulary"],
  "cards": {
    "file": "cards.json",
    "tier": 5,
    "card_types": ["Vocabulary"],
    "create_reverse": true
  }
}
```

| Field | Description |
|-------|-------------|
| `cards.file` | Path to cards JSON file (relative to pack dir) |
| `cards.tier` | Default tier for cards (1-5) |
| `cards.card_types` | Array of card type names |
| `cards.create_reverse` | Whether reverse cards were auto-generated |

#### Audio Pack

```json
{
  "id": "htsk-audio-lesson1",
  "name": "HTSK Lesson 1 Audio",
  "type": "audio",
  "audio": {
    "enhances": ["lesson1"],
    "structure": {
      "rows": "rows/row_{romanization}.mp3",
      "columns": "columns/col_{romanization}.mp3",
      "syllables": "syllables/{romanization}.mp3"
    }
  }
}
```

#### Generator Pack

```json
{
  "id": "htsk-scraper",
  "name": "HTSK Audio Scraper",
  "type": "generator",
  "generator": {
    "command": "uv run kr-scraper",
    "subcommands": [
      {"id": "lesson1", "args": [], "output": "lesson1/"},
      {"id": "lesson2", "args": [], "output": "lesson2/"},
      {"id": "lesson3", "args": [], "output": "lesson3/"}
    ],
    "output_type": "audio"
  }
}
```

---

## Pack Locations

### Default Locations

| Scope | Path | Description |
|-------|------|-------------|
| Shared | `data/content/packs/` | Admin-installed packs, available to all users |
| User | `data/users/{username}/content/packs/` | User-specific packs |

### Directory Structure

```
data/
├── content/
│   ├── packs/                      # Shared packs
│   │   ├── baseline/
│   │   │   ├── pack.json
│   │   │   └── cards.json
│   │   ├── my-vocabulary/          # Custom vocabulary pack
│   │   │   ├── pack.json
│   │   │   ├── vocabulary.json     # Source data (optional)
│   │   │   └── cards.json          # Card definitions
│   │   └── htsk-scraper/           # Pronunciation audio scraper
│   │       └── pack.json
│   └── generated/                  # Scraper output
│       └── htsk/
│           ├── lesson1/
│           ├── lesson2/
│           └── lesson3/
└── users/
    └── {username}/
        ├── learning.db
        └── content/
            ├── packs/              # User-specific packs
            └── generated/          # User-specific generated content
```

### Future: Custom Install Locations

Planned feature: Admin-configurable paths to external pack directories.

```
# Potential app_settings entry
pack_paths = /path/to/external/packs,/another/path
```

This would allow:
- Packs stored outside the data directory
- Git repositories mounted as pack sources
- Network-shared pack directories

---

## Pack Discovery

The `discover_packs()` function scans pack directories on demand.

```rust
pub fn discover_packs(
    shared_packs_dir: &Path,
    user_packs_dir: Option<&Path>,
    username: Option<&str>,
) -> Vec<PackLocation>
```

### Discovery Process

1. Scan shared packs directory (`data/content/packs/`)
2. Scan user packs directory if provided
3. For each subdirectory:
   - Look for `pack.json`
   - Parse manifest, log errors for invalid packs
   - Create `PackLocation` with scope and path

### PackLocation Structure

```rust
pub struct PackLocation {
    pub manifest: PackManifest,
    pub path: PathBuf,
    pub scope: PackScope,      // Shared or User
    pub username: Option<String>,
}

pub enum PackScope {
    Shared,
    User,
}
```

---

## Pack Installation Flow

### Enabling a Card Pack

```
User clicks "Enable" in Settings
            │
            ▼
┌─────────────────────────────────────┐
│  discover_packs()                   │
│  Find pack by ID                    │
└─────────────────────────────────────┘
            │
            ▼
┌─────────────────────────────────────┐
│  enable_card_pack()                 │
│  1. Register in content_packs       │
│  2. Load cards.json                 │
│  3. Insert card_definitions         │
│     (skip duplicates)               │
│  4. Record in enabled_packs         │
└─────────────────────────────────────┘
            │
            ▼
┌─────────────────────────────────────┐
│  Cards available for SRS study      │
└─────────────────────────────────────┘
```

### Disabling a Pack

- Removes entry from user's `enabled_packs` table
- Does NOT delete `card_definitions` (other users may have them)
- User's progress on those cards is preserved

### Deduplication

Cards are identified by: `(front, main_answer, card_type, tier, is_reverse)`

When enabling a pack:
- Existing cards with same signature are skipped
- New cards are inserted with `pack_id` reference
- Profiling logs skipped cards (with `--features profiling`)

---

## Admin Controls

| Feature | Status |
|---------|--------|
| Baseline pack always enabled | Implemented |
| Per-user pack enable/disable | Implemented |
| Pack visible to all users | Implemented (shared packs) |
| User roles (`user`, `admin`) | Implemented |
| User groups | Implemented |
| Group-based pack permissions | Implemented |

### User Roles

Users have a `role` column with values `'user'` (default) or `'admin'`.

**Admin determination (backwards compatible):**
1. User with `role = 'admin'` is admin
2. User with `username = 'admin'` is admin (legacy behavior preserved)
3. Check: `role = 'admin' OR LOWER(username) = 'admin'`

### User Groups

Groups allow organizing users for access control:

```sql
CREATE TABLE user_groups (
    id TEXT PRIMARY KEY,
    name TEXT NOT NULL,
    description TEXT,
    created_at TEXT NOT NULL
);

CREATE TABLE user_group_members (
    group_id TEXT NOT NULL,
    user_id INTEGER NOT NULL,
    added_at TEXT NOT NULL,
    PRIMARY KEY (group_id, user_id)
);
```

### Pack Permissions

Control which users/groups can access each pack:

```sql
CREATE TABLE pack_permissions (
    pack_id TEXT NOT NULL,
    group_id TEXT NOT NULL DEFAULT '',  -- '' = all users
    allowed INTEGER NOT NULL DEFAULT 1,
    PRIMARY KEY (pack_id, group_id)
);
```

**Permission resolution:**
1. Admins can access all packs
2. If no `pack_permissions` entry exists → pack available to all
3. If entry with `group_id = ''` and `allowed = 1` exists → available to all
4. Otherwise, user must be member of an allowed group

**Use cases:**
- Restrict premium content to specific groups
- Beta test new packs with "beta-testers" group
- Disable problematic packs without deleting

---

## Creating a New Pack

### 1. Create Directory Structure

```bash
mkdir -p data/content/packs/my-pack
```

### 2. Create pack.json

```json
{
  "id": "my-pack",
  "name": "My Custom Pack",
  "version": "1.0.0",
  "type": "cards",
  "description": "Custom flashcards",
  "provides": [],
  "cards": {
    "file": "cards.json",
    "tier": 3,
    "card_types": ["Custom"],
    "create_reverse": true
  }
}
```

Note: Add `"provides": ["vocabulary"]` if your pack includes a `vocabulary.json` file for the vocabulary library.

### 3. Create cards.json

```json
{
  "cards": [
    {
      "front": "Question",
      "main_answer": "Answer",
      "description": "Optional description",
      "card_type": "Custom",
      "tier": 3,
      "is_reverse": false,
      "audio_hint": null
    }
  ]
}
```

### 4. Test

1. Restart server (packs discovered on settings page load)
2. Go to Settings → Content Packs
3. Click Enable on your pack
4. Check logs for insert/skip counts

---

## Database Schema

### content_packs (app.db)

Shared registry of installed packs.

| Column | Type | Description |
|--------|------|-------------|
| `id` | TEXT PK | Pack identifier |
| `name` | TEXT | Display name |
| `pack_type` | TEXT | `cards`, `audio`, `generator` |
| `version` | TEXT | Semver version |
| `description` | TEXT | Brief description |
| `source_path` | TEXT | Path to pack directory |
| `scope` | TEXT | `shared` or `user` |
| `installed_at` | TEXT | ISO 8601 timestamp |
| `installed_by` | TEXT | Username who installed |
| `metadata` | TEXT | JSON configuration |

### card_definitions (app.db)

Shared card content, referenced by all users.

| Column | Type | Description |
|--------|------|-------------|
| `id` | INTEGER PK | Card ID |
| `front` | TEXT | Question/prompt |
| `main_answer` | TEXT | Expected answer |
| `description` | TEXT | Optional notes |
| `card_type` | TEXT | Card category |
| `tier` | INTEGER | Learning tier (1-5) |
| `audio_hint` | TEXT | Audio file path |
| `is_reverse` | INTEGER | 0=forward, 1=reverse |
| `pack_id` | TEXT FK | Source pack (NULL=baseline) |

### enabled_packs (learning.db)

Per-user pack enablement.

| Column | Type | Description |
|--------|------|-------------|
| `pack_id` | TEXT PK | Pack identifier |
| `enabled_at` | TEXT | ISO 8601 timestamp |
| `cards_created` | INTEGER | 1 if cards were created |
| `config` | TEXT | User-specific JSON config |

### user_groups (app.db)

User groups for organizing access control.

| Column | Type | Description |
|--------|------|-------------|
| `id` | TEXT PK | Group identifier (e.g., `beta-testers`) |
| `name` | TEXT | Display name |
| `description` | TEXT | Group description |
| `created_at` | TEXT | ISO 8601 timestamp |

### user_group_members (app.db)

Group membership (many-to-many).

| Column | Type | Description |
|--------|------|-------------|
| `group_id` | TEXT PK | Group identifier |
| `user_id` | INTEGER PK | User ID |
| `added_at` | TEXT | ISO 8601 timestamp |

### pack_permissions (app.db)

Pack access control per group.

| Column | Type | Description |
|--------|------|-------------|
| `pack_id` | TEXT PK | Pack identifier |
| `group_id` | TEXT PK | Group ('' = all users) |
| `allowed` | INTEGER | 1 = allowed, 0 = blocked |

### users.role column (app.db)

User role for admin determination.

| Value | Description |
|-------|-------------|
| `user` | Default role, standard permissions |
| `admin` | Full admin access (can manage packs, users, groups) |

---

## Key Source Files

| File | Purpose |
|------|---------|
| `src/content/discovery.rs` | Pack scanning, discovery, and content type queries (`any_pack_provides`, `find_packs_providing`) |
| `src/content/packs.rs` | PackManifest structures and parsing |
| `src/content/cards.rs` | Card pack enable/disable logic |
| `src/content/audio.rs` | Audio pack resolution |
| `src/content/generator.rs` | Generator execution |
| `src/paths.rs` | Directory path constants |
| `src/auth/db.rs` | Shared database schema |
| `src/db/schema.rs` | User database schema |
