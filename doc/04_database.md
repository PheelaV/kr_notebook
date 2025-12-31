# Database Schema

This document describes the SQLite database schema used by kr_notebook.

## Overview

- **Database**: SQLite 3
- **Location**: `data/users/<username>/hangul.db` (per-user databases)
- **Auth Database**: `data/auth.db` (user accounts and sessions)
- **Schema**: `src/db/schema.rs`
- **Migrations**: Inline in `run_migrations()` - columns added if missing, runs on each connection

## Entity Relationship Diagram

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
                    │                 cards                    │
                    │                                          │
                    │  id (PK)                                 │
                    │  front, main_answer, description         │
                    │  card_type, tier, audio_hint, is_reverse │
                    │  ease_factor, interval_days, repetitions │
                    │  fsrs_stability, fsrs_difficulty         │
                    │  fsrs_state, learning_step               │
                    │  next_review                             │
                    │  total_reviews, correct_reviews          │
                    └─────────────────────────────────────────┘

┌─────────────────────┐         ┌─────────────────────────────┐
│  character_stats    │         │  tier_graduation_backups    │
│                     │         │                             │
│  character (PK)     │         │  tier (PK)                  │
│  character_type     │         │  backup_data (JSON)         │
│  total_attempts     │         │  created_at                 │
│  total_correct      │         └─────────────────────────────┘
│  attempts_7d        │
│  correct_7d         │
│  attempts_1d        │
│  correct_1d         │
│  last_attempt_at    │
└─────────────────────┘
```

## Tables

### `cards`

Core flashcard table with content and SRS scheduling data.

| Column | Type | Constraints | Description |
|--------|------|-------------|-------------|
| `id` | INTEGER | PRIMARY KEY AUTOINCREMENT | Unique card ID |
| `front` | TEXT | NOT NULL | Card front (question side) |
| `main_answer` | TEXT | NOT NULL | Primary answer |
| `description` | TEXT | | Optional description/notes |
| `card_type` | TEXT | NOT NULL | `Consonant`, `Vowel`, `Syllable` |
| `tier` | INTEGER | NOT NULL | Learning tier (1-4) |
| `audio_hint` | TEXT | | Path to audio file |
| `is_reverse` | INTEGER | NOT NULL DEFAULT 0 | 0=Korean→Rom, 1=Rom→Korean |
| `ease_factor` | REAL | NOT NULL DEFAULT 2.5 | SM-2 ease factor |
| `interval_days` | INTEGER | NOT NULL DEFAULT 0 | SM-2 interval |
| `repetitions` | INTEGER | NOT NULL DEFAULT 0 | Successful review count |
| `next_review` | TEXT | NOT NULL | ISO 8601 datetime |
| `total_reviews` | INTEGER | NOT NULL DEFAULT 0 | Total review attempts |
| `correct_reviews` | INTEGER | NOT NULL DEFAULT 0 | Correct attempts |
| `learning_step` | INTEGER | NOT NULL DEFAULT 0 | Learning phase (0-3), graduated at 4 |
| `fsrs_stability` | REAL | | FSRS memory stability (days) |
| `fsrs_difficulty` | REAL | | FSRS difficulty (0-10) |
| `fsrs_state` | TEXT | DEFAULT 'New' | `New`, `Learning`, `Review`, `Relearning` |

### `review_logs`

Review history for analytics, debugging, and FSRS parameter training.

| Column | Type | Constraints | Description |
|--------|------|-------------|-------------|
| `id` | INTEGER | PRIMARY KEY AUTOINCREMENT | Log entry ID |
| `card_id` | INTEGER | NOT NULL, FK → cards(id) | Reviewed card |
| `quality` | INTEGER | NOT NULL | Rating (0=Again, 2=Hard, 4=Good, 5=Easy) |
| `reviewed_at` | TEXT | NOT NULL | ISO 8601 datetime |
| `is_correct` | INTEGER | | 0=incorrect, 1=correct |
| `study_mode` | TEXT | | `Classic`, `Interactive`, `Listening`, `PracticeFlip`, `PracticeInteractive` |
| `direction` | TEXT | | `KrToRom`, `RomToKr`, `AudioToKr` |
| `response_time_ms` | INTEGER | | Time to answer in milliseconds |
| `hints_used` | INTEGER | DEFAULT 0 | Number of hints requested |

### `settings`

Key-value store for application configuration.

| Column | Type | Constraints | Description |
|--------|------|-------------|-------------|
| `key` | TEXT | PRIMARY KEY | Setting name |
| `value` | TEXT | NOT NULL | Setting value (as string) |

### `confusions`

Tracks frequently confused answers for targeted review.

| Column | Type | Constraints | Description |
|--------|------|-------------|-------------|
| `id` | INTEGER | PRIMARY KEY AUTOINCREMENT | Entry ID |
| `card_id` | INTEGER | NOT NULL, FK → cards(id) | Card that was confused |
| `wrong_answer` | TEXT | NOT NULL | The incorrect answer given |
| `count` | INTEGER | NOT NULL DEFAULT 1 | Times this confusion occurred |
| `last_confused_at` | TEXT | NOT NULL | ISO 8601 datetime |

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
| `last_attempt_at` | TEXT | | ISO 8601 datetime |

### `tier_graduation_backups`

Stores card state before tier graduation for undo capability.

| Column | Type | Constraints | Description |
|--------|------|-------------|-------------|
| `tier` | INTEGER | PRIMARY KEY | Tier number (1-4) |
| `backup_data` | TEXT | NOT NULL | JSON array of card states |
| `created_at` | TEXT | NOT NULL | ISO 8601 datetime |

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

## Indexes

| Index | Table | Column(s) | Purpose |
|-------|-------|-----------|---------|
| `idx_cards_next_review` | cards | next_review | Fast due card queries |
| `idx_cards_tier` | cards | tier | Filter by tier |
| `idx_review_logs_card_id` | review_logs | card_id | Card history lookup |
| `idx_review_logs_reviewed_at` | review_logs | reviewed_at | Time-based queries |
| `idx_review_logs_study_mode` | review_logs | study_mode | Mode analytics |
| `idx_confusions_card_id` | confusions | card_id | Card confusion lookup |
| `idx_character_stats_type` | character_stats | character_type | Type filtering |

## Settings Reference

| Key | Default | Description |
|-----|---------|-------------|
| `max_unlocked_tier` | `1` | Highest unlocked tier (progressive mode) |
| `dark_mode` | `false` | UI dark mode (stored but uses localStorage) |
| `tts_enabled` | `true` | Text-to-speech enabled |
| `tts_model` | `mms` | TTS model to use |
| `all_tiers_unlocked` | `false` | Accelerated mode (all tiers available) |
| `enabled_tiers` | `1,2,3,4` | Comma-separated enabled tiers |
| `desired_retention` | `0.9` | FSRS target retention (0.8-0.95) |
| `use_fsrs` | `true` | Use FSRS algorithm (vs SM-2) |
| `use_interleaving` | `true` | Mix card types during study |
| `focus_tier` | *(none)* | Single tier focus mode (1-4 or absent) |

## Migrations

Migrations run automatically on every database connection via `run_migrations()`. They are idempotent (safe to run multiple times).

| Migration | Description |
|-----------|-------------|
| Add `learning_step` | Learning step tracking for graduated learning |
| Add FSRS columns | `fsrs_stability`, `fsrs_difficulty`, `fsrs_state` |
| Add enhanced review logging | `is_correct`, `study_mode`, `direction`, `response_time_ms`, `hints_used` |
| Add `is_reverse` | Explicit card direction tracking (replaces string parsing) |
| Backfill `is_correct` | Derives from quality rating for existing logs |
| Backfill `is_reverse` | Detects reverse cards from legacy front text pattern |
