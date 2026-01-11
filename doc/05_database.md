# Database Schema

This document describes the SQLite database schema used by kr_notebook.

## Overview

kr_notebook uses a **dual-database architecture**:

| Database | Location | Purpose |
|----------|----------|---------|
| **app.db** | `data/app.db` | Shared: users, sessions, card definitions, content packs, permissions |
| **learning.db** | `data/users/<username>/learning.db` | Per-user: SRS progress, reviews, statistics, preferences |

- **Database**: SQLite 3
- **app.db schema**: `src/auth/db.rs` (version 9)
- **learning.db schema**: `src/db/schema.rs` (version 5)
- **Cross-database queries**: Uses `ATTACH DATABASE` to join card definitions with user progress

## Entity Relationship Diagrams

### app.db (Shared)

```
┌─────────────────────┐         ┌─────────────────────────────┐
│     app_settings    │         │        content_packs        │
│   (key-value store) │         │                             │
└─────────────────────┘         │  id (PK)                    │
                                │  name, pack_type, version   │
┌─────────────────────┐         │  description, source_path   │
│     db_version      │         │  scope, installed_at        │
│  (schema tracking)  │         │  installed_by, metadata     │
└─────────────────────┘         └──────────────┬──────────────┘
                                               │
┌─────────────────────┐                        │
│       users         │                        ▼
│                     │         ┌─────────────────────────────┐
│  id (PK)            │         │      card_definitions       │
│  username           │         │                             │
│  password_hash      │         │  id (PK)                    │
│  role               │         │  front, main_answer         │
│  is_guest           │         │  description, card_type     │
│  created_at         │         │  tier, audio_hint           │
│  last_login_at      │         │  is_reverse                 │
│  last_activity_at   │         │  pack_id ────────────────────
└─────────┬───────────┘         └─────────────────────────────┘
          │
          ├───────────────────────────┐
          │                           │
          ▼                           ▼
┌─────────────────────┐   ┌───────────────────────┐
│     sessions        │   │  user_group_members   │
│                     │   │                       │
│  id (PK)            │   │  group_id, user_id    │◄────┐
│  user_id (FK)       │   │  added_at             │     │
│  created_at         │   └───────────────────────┘     │
│  expires_at         │                                 │
│  last_access_at     │   ┌───────────────────────┐     │
└─────────────────────┘   │     user_groups       │     │
                          │                       │     │
                          │  id (PK) ─────────────┼─────┘
                          │  name, description    │
                          │  created_at           │
                          └───────────┬───────────┘
                                      │
          ┌───────────────────────────┘
          ▼
┌─────────────────────────┐   ┌─────────────────────────┐
│   pack_permissions      │   │  pack_user_permissions  │
│                         │   │                         │
│  pack_id, group_id (PK) │   │  pack_id, user_id (PK)  │
│  allowed                │   │  allowed                │
└─────────────────────────┘   └─────────────────────────┘

┌─────────────────────────┐   ┌─────────────────────────┐
│  registered_pack_paths  │   │    pack_ui_metadata     │
│                         │   │                         │
│  id (PK), path          │   │  pack_id (PK)           │
│  name, registered_by    │   │  display_name, unit_name│
│  registered_at          │   │  unlock_threshold       │
│  is_active              │   │  total_lessons          │
└─────────────────────────┘   └─────────────────────────┘
```

### learning.db (Per-User)

```
                          ┌─────────────────────┐
                          │      settings       │
                          │  (key-value store)  │
                          └─────────────────────┘

┌─────────────────────┐         ┌─────────────────────┐
│   review_logs       │         │    confusions       │
│                     │         │                     │
│  id                 │         │  id                 │
│  card_id ───────────┼────┐    │  card_id ───────────┼────┐
│  quality            │    │    │  wrong_answer       │    │
│  reviewed_at        │    │    │  count              │    │
│  is_correct         │    │    │  last_confused_at   │    │
│  study_mode         │    │    └─────────────────────┘    │
│  direction          │    │                               │
│  response_time_ms   │    │                               │
│  hints_used         │    │                               │
└─────────────────────┘    │                               │
                           │                               │
                           ▼                               ▼
                    ┌─────────────────────────────────────────┐
                    │            cards (LEGACY)               │
                    │                                         │
                    │  id (PK)                                │
                    │  front, main_answer, description        │
                    │  card_type, tier, audio_hint, is_reverse│
                    │  ease_factor, interval_days, repetitions│
                    │  fsrs_stability, fsrs_difficulty        │
                    │  fsrs_state, learning_step              │
                    │  next_review, total_reviews             │
                    │  correct_reviews, pack_id               │
                    └─────────────────────────────────────────┘

┌─────────────────────────────┐   ┌─────────────────────────────┐
│       card_progress         │   │       enabled_packs         │
│                             │   │                             │
│  card_id (PK) ──────────────┼──►│  pack_id (PK)               │
│  ease_factor, interval_days │   │  enabled_at                 │
│  repetitions, next_review   │   │  cards_created              │
│  total_reviews, correct     │   │  config (JSON)              │
│  learning_step              │   └─────────────────────────────┘
│  fsrs_stability, difficulty │
│  fsrs_state                 │   ┌─────────────────────────────┐
└─────────────────────────────┘   │   pack_lesson_progress      │
                                  │                             │
┌─────────────────────┐           │  pack_id, lesson (PK)       │
│  character_stats    │           │  unlocked, unlocked_at      │
│                     │           └─────────────────────────────┘
│  character (PK)     │
│  character_type     │           ┌─────────────────────────────┐
│  total_attempts     │           │  tier_graduation_backups    │
│  total_correct      │           │                             │
│  attempts_7d        │           │  tier (PK)                  │
│  correct_7d         │           │  backup_data (JSON)         │
│  attempts_1d        │           │  created_at                 │
│  correct_1d         │           └─────────────────────────────┘
│  last_attempt_at    │
└─────────────────────┘           ┌─────────────────────┐
                                  │     db_version      │
                                  │  (schema tracking)  │
                                  └─────────────────────┘

Cross-database relationship:
  card_progress.card_id ──► app.db.card_definitions.id
```

## app.db Tables

### `db_version`

Schema version tracking for migrations.

| Column | Type | Constraints | Description |
|--------|------|-------------|-------------|
| `version` | INTEGER | PRIMARY KEY | Migration version number |
| `applied_at` | TEXT | NOT NULL | RFC3339 timestamp |
| `description` | TEXT | | Migration description |

### `users`

User accounts for the learning system.

| Column | Type | Constraints | Description |
|--------|------|-------------|-------------|
| `id` | INTEGER | PRIMARY KEY AUTOINCREMENT | Unique user ID |
| `username` | TEXT | NOT NULL, UNIQUE, COLLATE NOCASE | Login name (case-insensitive) |
| `password_hash` | TEXT | NOT NULL | Argon2 password hash |
| `created_at` | TEXT | NOT NULL | RFC3339 timestamp |
| `last_login_at` | TEXT | | Last login timestamp |
| `is_guest` | INTEGER | DEFAULT 0 | 1 if guest account |
| `last_activity_at` | TEXT | | Last activity (for guest expiry) |
| `role` | TEXT | DEFAULT 'user' | `user` or `admin` |

### `sessions`

Active user sessions for authentication.

| Column | Type | Constraints | Description |
|--------|------|-------------|-------------|
| `id` | TEXT | PRIMARY KEY | Session token |
| `user_id` | INTEGER | NOT NULL, FK → users(id) | Session owner |
| `created_at` | TEXT | NOT NULL | Session creation time |
| `expires_at` | TEXT | NOT NULL | Session expiration time |
| `last_access_at` | TEXT | NOT NULL | Last activity time |

### `app_settings`

Global application configuration.

| Column | Type | Constraints | Description |
|--------|------|-------------|-------------|
| `key` | TEXT | PRIMARY KEY | Setting name |
| `value` | TEXT | | Setting value (nullable) |

**Default settings:**
| Key | Default | Description |
|-----|---------|-------------|
| `max_users` | NULL | User registration limit (NULL = unlimited) |
| `max_guests` | NULL | Guest account limit (NULL = unlimited) |
| `guest_expiry_hours` | `24` | Hours before inactive guests are deleted |

### `content_packs`

Registry of installed card packs.

| Column | Type | Constraints | Description |
|--------|------|-------------|-------------|
| `id` | TEXT | PRIMARY KEY | Pack unique identifier |
| `name` | TEXT | NOT NULL | Human-readable name |
| `pack_type` | TEXT | NOT NULL | `audio`, `generator`, or `cards` |
| `version` | TEXT | | Semantic version string |
| `description` | TEXT | | Pack description |
| `source_path` | TEXT | NOT NULL | Relative path to pack directory |
| `scope` | TEXT | NOT NULL | `shared` or `user` |
| `installed_at` | TEXT | NOT NULL | RFC3339 installation timestamp |
| `installed_by` | TEXT | | Username of installer (NULL for shared) |
| `metadata` | TEXT | | JSON: type-specific configuration |
| `is_enabled` | INTEGER | DEFAULT 1 | 1 = enabled, 0 = disabled globally |

### `card_definitions`

Shared card content (Korean language learning cards).

| Column | Type | Constraints | Description |
|--------|------|-------------|-------------|
| `id` | INTEGER | PRIMARY KEY AUTOINCREMENT | Unique card ID |
| `front` | TEXT | NOT NULL | Question/prompt (e.g., Korean character) |
| `main_answer` | TEXT | NOT NULL | Primary answer (e.g., romanization) |
| `description` | TEXT | | Learning hint or explanation |
| `card_type` | TEXT | NOT NULL | `Consonant`, `Vowel`, `AspiratedConsonant`, `TenseConsonant`, `CompoundVowel` |
| `tier` | INTEGER | NOT NULL | Learning tier (1-4) |
| `audio_hint` | TEXT | | Path to audio file |
| `is_reverse` | INTEGER | NOT NULL DEFAULT 0 | 1 for reverse cards (answer→question) |
| `pack_id` | TEXT | FK → content_packs(id) | NULL for baseline cards |
| `lesson` | INTEGER | | Lesson number within pack (for lesson-based packs) |

### `user_groups`

Role-based access control groups.

| Column | Type | Constraints | Description |
|--------|------|-------------|-------------|
| `id` | TEXT | PRIMARY KEY | Group identifier |
| `name` | TEXT | NOT NULL | Display name |
| `description` | TEXT | | Group description |
| `created_at` | TEXT | NOT NULL | RFC3339 creation timestamp |

### `user_group_members`

Many-to-many relationship between users and groups.

| Column | Type | Constraints | Description |
|--------|------|-------------|-------------|
| `group_id` | TEXT | NOT NULL, FK → user_groups(id) | Group reference |
| `user_id` | INTEGER | NOT NULL, FK → users(id) | User reference |
| `added_at` | TEXT | NOT NULL | RFC3339 timestamp |

**Primary key:** `(group_id, user_id)`

### `pack_permissions`

Group-level pack access control.

| Column | Type | Constraints | Description |
|--------|------|-------------|-------------|
| `pack_id` | TEXT | NOT NULL, FK → content_packs(id) | Pack reference |
| `group_id` | TEXT | NOT NULL DEFAULT '' | Group ID (empty = all users) |
| `allowed` | INTEGER | NOT NULL DEFAULT 1 | 1 = allow, 0 = deny |

**Primary key:** `(pack_id, group_id)`

### `pack_user_permissions`

User-level direct pack access control.

| Column | Type | Constraints | Description |
|--------|------|-------------|-------------|
| `pack_id` | TEXT | NOT NULL, FK → content_packs(id) | Pack reference |
| `user_id` | INTEGER | NOT NULL, FK → users(id) | User reference |
| `allowed` | INTEGER | NOT NULL DEFAULT 1 | 1 = allow, 0 = deny |

**Primary key:** `(pack_id, user_id)`

### `registered_pack_paths`

External pack directory registration for admin-managed content.

| Column | Type | Constraints | Description |
|--------|------|-------------|-------------|
| `id` | INTEGER | PRIMARY KEY AUTOINCREMENT | Entry ID |
| `path` | TEXT | NOT NULL, UNIQUE | Filesystem path to pack directory |
| `name` | TEXT | | Display name for the path |
| `registered_by` | TEXT | NOT NULL | Username who registered the path |
| `registered_at` | TEXT | NOT NULL | RFC3339 timestamp |
| `is_active` | INTEGER | DEFAULT 1 | 1 = active, 0 = inactive |

### `pack_ui_metadata`

UI configuration for lesson-based pack progression.

| Column | Type | Constraints | Description |
|--------|------|-------------|-------------|
| `pack_id` | TEXT | PRIMARY KEY, FK → content_packs(id) | Pack reference |
| `display_name` | TEXT | NOT NULL | Name shown in UI |
| `unit_name` | TEXT | DEFAULT 'Lessons' | Label for lesson groups (e.g., "Units", "Lessons") |
| `section_prefix` | TEXT | DEFAULT 'Lesson' | Prefix for individual lessons (e.g., "Lesson 1") |
| `lesson_labels` | TEXT | | JSON array of custom lesson labels |
| `unlock_threshold` | INTEGER | DEFAULT 80 | Percentage required to unlock next lesson |
| `total_lessons` | INTEGER | | Total number of lessons in pack |
| `progress_section_title` | TEXT | | Title for progress section |
| `study_filter_label` | TEXT | | Label for study filter dropdown |

## learning.db Tables

### `db_version`

Schema version tracking (same structure as app.db).

### `card_progress`

SRS state for individual cards (references app.db.card_definitions).

| Column | Type | Constraints | Description |
|--------|------|-------------|-------------|
| `card_id` | INTEGER | PRIMARY KEY | FK → app.db.card_definitions(id) |
| `ease_factor` | REAL | NOT NULL DEFAULT 2.5 | SM-2 ease factor |
| `interval_days` | INTEGER | NOT NULL DEFAULT 0 | Days until next review |
| `repetitions` | INTEGER | NOT NULL DEFAULT 0 | Successful review count |
| `next_review` | TEXT | NOT NULL | RFC3339 datetime |
| `total_reviews` | INTEGER | NOT NULL DEFAULT 0 | Total review attempts |
| `correct_reviews` | INTEGER | NOT NULL DEFAULT 0 | Correct attempts |
| `learning_step` | INTEGER | NOT NULL DEFAULT 0 | Learning phase (0-3), graduated at 4 |
| `fsrs_stability` | REAL | | FSRS memory stability (days) |
| `fsrs_difficulty` | REAL | | FSRS difficulty (0-10) |
| `fsrs_state` | TEXT | DEFAULT 'New' | `New`, `Learning`, `Review`, `Relearning` |

### `cards` (LEGACY)

**Note:** This table is kept for backward compatibility. New progress is stored in `card_progress`. Will be removed in a future version.

| Column | Type | Constraints | Description |
|--------|------|-------------|-------------|
| `id` | INTEGER | PRIMARY KEY AUTOINCREMENT | Unique card ID |
| `front` | TEXT | NOT NULL | Card front (question side) |
| `main_answer` | TEXT | NOT NULL | Primary answer |
| `description` | TEXT | | Optional description/notes |
| `card_type` | TEXT | NOT NULL | `Consonant`, `Vowel`, etc. |
| `tier` | INTEGER | NOT NULL | Learning tier (1-4) |
| `audio_hint` | TEXT | | Path to audio file |
| `is_reverse` | INTEGER | NOT NULL DEFAULT 0 | 0=Korean→Rom, 1=Rom→Korean |
| `ease_factor` | REAL | NOT NULL DEFAULT 2.5 | SM-2 ease factor |
| `interval_days` | INTEGER | NOT NULL DEFAULT 0 | SM-2 interval |
| `repetitions` | INTEGER | NOT NULL DEFAULT 0 | Successful review count |
| `next_review` | TEXT | NOT NULL | RFC3339 datetime |
| `total_reviews` | INTEGER | NOT NULL DEFAULT 0 | Total review attempts |
| `correct_reviews` | INTEGER | NOT NULL DEFAULT 0 | Correct attempts |
| `learning_step` | INTEGER | NOT NULL DEFAULT 0 | Learning phase (0-3) |
| `fsrs_stability` | REAL | | FSRS memory stability |
| `fsrs_difficulty` | REAL | | FSRS difficulty |
| `fsrs_state` | TEXT | DEFAULT 'New' | FSRS state |
| `pack_id` | TEXT | | Pack that created this card (NULL = baseline) |

### `review_logs`

Review history for analytics and FSRS parameter training.

| Column | Type | Constraints | Description |
|--------|------|-------------|-------------|
| `id` | INTEGER | PRIMARY KEY AUTOINCREMENT | Log entry ID |
| `card_id` | INTEGER | NOT NULL, FK → cards(id) | Reviewed card |
| `quality` | INTEGER | NOT NULL | Rating (0=Again, 2=Hard, 4=Good, 5=Easy) |
| `reviewed_at` | TEXT | NOT NULL | RFC3339 datetime |
| `is_correct` | INTEGER | | 0=incorrect, 1=correct |
| `study_mode` | TEXT | | `Classic`, `Interactive`, `Listening`, `PracticeFlip`, `PracticeInteractive` |
| `direction` | TEXT | | `KrToRom`, `RomToKr`, `AudioToKr` |
| `response_time_ms` | INTEGER | | Time to answer in milliseconds |
| `hints_used` | INTEGER | DEFAULT 0 | Number of hints requested |

### `settings`

User-specific learning preferences.

| Column | Type | Constraints | Description |
|--------|------|-------------|-------------|
| `key` | TEXT | PRIMARY KEY | Setting name |
| `value` | TEXT | NOT NULL | Setting value (as string) |

**Default settings:**
| Key | Default | Description |
|-----|---------|-------------|
| `max_unlocked_tier` | `1` | Highest unlocked tier (progressive mode) |
| `dark_mode` | `false` | UI dark mode |
| `tts_enabled` | `true` | Text-to-speech enabled |
| `tts_model` | `mms` | TTS model to use |
| `all_tiers_unlocked` | `false` | Accelerated mode (all tiers available) |
| `enabled_tiers` | `1,2,3,4` | Comma-separated enabled tiers |
| `desired_retention` | `0.9` | FSRS target retention (0.8-0.95) |
| `use_fsrs` | `true` | Use FSRS algorithm (vs SM-2) |
| `use_interleaving` | `true` | Mix card types during study |
| `focus_tier` | *(none)* | Single tier focus mode (1-4 or absent) |
| `accelerated_packs` | *(empty)* | Comma-separated pack IDs in accelerated mode |
| `study_filter_mode` | `all` | Study filter: `all`, `pack`, `lesson` |
| `study_filter_pack` | *(empty)* | Pack ID for filtered study |
| `study_filter_lessons` | *(empty)* | Comma-separated lesson numbers for filtered study |

### `confusions`

Tracks frequently confused answers for targeted review.

| Column | Type | Constraints | Description |
|--------|------|-------------|-------------|
| `id` | INTEGER | PRIMARY KEY AUTOINCREMENT | Entry ID |
| `card_id` | INTEGER | NOT NULL, FK → cards(id) | Card that was confused |
| `wrong_answer` | TEXT | NOT NULL | The incorrect answer given |
| `count` | INTEGER | NOT NULL DEFAULT 1 | Times this confusion occurred |
| `last_confused_at` | TEXT | NOT NULL | RFC3339 datetime |

### `character_stats`

Per-character accuracy statistics with rolling windows.

| Column | Type | Constraints | Description |
|--------|------|-------------|-------------|
| `character` | TEXT | PRIMARY KEY | Korean character |
| `character_type` | TEXT | NOT NULL | `consonant`, `vowel`, `syllable` |
| `total_attempts` | INTEGER | DEFAULT 0 | All-time attempts |
| `total_correct` | INTEGER | DEFAULT 0 | All-time correct |
| `attempts_7d` | INTEGER | DEFAULT 0 | Last 7 days attempts |
| `correct_7d` | INTEGER | DEFAULT 0 | Last 7 days correct |
| `attempts_1d` | INTEGER | DEFAULT 0 | Last 24 hours attempts |
| `correct_1d` | INTEGER | DEFAULT 0 | Last 24 hours correct |
| `last_attempt_at` | TEXT | | RFC3339 datetime |

### `tier_graduation_backups`

Stores card state before tier graduation for undo capability.

| Column | Type | Constraints | Description |
|--------|------|-------------|-------------|
| `tier` | INTEGER | PRIMARY KEY | Tier number (1-4) |
| `backup_data` | TEXT | NOT NULL | JSON array of card states |
| `created_at` | TEXT | NOT NULL | RFC3339 datetime |

**backup_data JSON structure:**
```json
[
  {
    "id": 123,
    "learning_step": 2,
    "repetitions": 0,
    "fsrs_stability": null,
    "fsrs_difficulty": null,
    "fsrs_state": "Learning",
    "next_review": "2024-12-28T10:00:00Z"
  }
]
```

### `enabled_packs`

Tracks which content packs are enabled for this user.

| Column | Type | Constraints | Description |
|--------|------|-------------|-------------|
| `pack_id` | TEXT | PRIMARY KEY | FK → app.db.content_packs(id) |
| `enabled_at` | TEXT | NOT NULL | RFC3339 timestamp |
| `cards_created` | INTEGER | DEFAULT 0 | 1 if cards were created from this pack |
| `config` | TEXT | | JSON: user-specific pack settings |

### `pack_lesson_progress`

Tracks lesson unlock progress for lesson-based packs.

| Column | Type | Constraints | Description |
|--------|------|-------------|-------------|
| `pack_id` | TEXT | NOT NULL | FK → app.db.content_packs(id) |
| `lesson` | INTEGER | NOT NULL | Lesson number |
| `unlocked` | INTEGER | NOT NULL DEFAULT 0 | 1 if unlocked |
| `unlocked_at` | TEXT | | RFC3339 timestamp when unlocked |

**Primary key:** `(pack_id, lesson)`

## Indexes

### app.db Indexes

| Index | Table | Column(s) | Purpose |
|-------|-------|-----------|---------|
| `idx_users_is_guest` | users | is_guest | Filter guest accounts |
| `idx_sessions_user_id` | sessions | user_id | User session lookup |
| `idx_sessions_expires_at` | sessions | expires_at | Expired session cleanup |
| `idx_content_packs_scope` | content_packs | scope | Filter by scope |
| `idx_content_packs_type` | content_packs | pack_type | Filter by type |
| `idx_card_definitions_pack` | card_definitions | pack_id | Cards by pack |
| `idx_card_definitions_tier` | card_definitions | tier | Cards by tier |
| `idx_user_group_members_user` | user_group_members | user_id | User's groups |
| `idx_pack_permissions_group` | pack_permissions | group_id | Group permissions |
| `idx_pack_user_permissions_user` | pack_user_permissions | user_id | User permissions |
| `idx_card_definitions_pack_lesson` | card_definitions | pack_id, lesson | Cards by pack and lesson |

### learning.db Indexes

| Index | Table | Column(s) | Purpose |
|-------|-------|-----------|---------|
| `idx_cards_next_review` | cards | next_review | Fast due card queries |
| `idx_cards_tier` | cards | tier | Filter by tier |
| `idx_cards_pack_id` | cards | pack_id | Cards by pack |
| `idx_card_progress_next_review` | card_progress | next_review | Fast due card queries |
| `idx_review_logs_card_id` | review_logs | card_id | Card history lookup |
| `idx_review_logs_reviewed_at` | review_logs | reviewed_at | Time-based queries |
| `idx_review_logs_study_mode` | review_logs | study_mode | Mode analytics |
| `idx_confusions_card_id` | confusions | card_id | Card confusion lookup |
| `idx_character_stats_type` | character_stats | character_type | Type filtering |

## Migrations

Migrations run automatically on database connection via versioned migration functions. They are idempotent using:
- `CREATE TABLE IF NOT EXISTS` for new tables
- `add_column_if_missing()` for new columns
- `db_version` table to track applied migrations

### app.db Migrations (AUTH_DB_VERSION = 9)

| Version | Description |
|---------|-------------|
| 1 | Initial schema (users, sessions, app_settings) |
| 2 | Add guest columns (is_guest, last_activity_at) |
| 3 | Add content packs system (content_packs, card_definitions) |
| 4 | Add user roles and groups (role column, user_groups, pack_permissions) |
| 5 | Add user-level pack permissions (pack_user_permissions) |
| 6 | Add external pack path registration (registered_pack_paths) |
| 7 | Add lesson-based pack progression (pack_ui_metadata, card_definitions.lesson) |
| 8 | Add global pack enable/disable (content_packs.is_enabled) |
| 9 | Register baseline pack and add public permissions |

### learning.db Migrations (LEARNING_DB_VERSION = 5)

| Version | Description |
|---------|-------------|
| 1 | Initial schema (cards, review_logs, settings, confusions, character_stats, tier_graduation_backups) |
| 2 | Add FSRS columns, enhanced review logging, is_reverse, pack_id |
| 3 | Add card_progress and enabled_packs tables |
| 4 | Add content pack support (cards.pack_id, enabled_packs table) |
| 5 | Add pack lesson progression (pack_lesson_progress, study filter settings) |

### Legacy Migration: cards → card_progress

When opening a learning.db with legacy `cards` data, the system:
1. Attaches app.db to access `card_definitions`
2. Matches legacy cards to definitions by content (front, main_answer, card_type, tier, is_reverse)
3. Copies SRS progress to `card_progress`
4. Preserves the legacy `cards` table for review_logs foreign keys
