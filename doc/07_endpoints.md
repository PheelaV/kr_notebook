# API Endpoints

Complete reference for all HTTP endpoints in the Korean Hangul Learning App.

## Overview

| Category | Count | Auth Required |
|----------|-------|---------------|
| Authentication | 7 | No |
| Reference/Guide | 8 | No |
| Home & Navigation | 1 | Yes |
| Study Modes | 9 | Yes |
| Listening Practice | 5 | Yes |
| Progress & Library | 5 | Yes |
| Settings | 4 | Yes |
| Content Packs | 7 | Yes (some admin) |
| Admin: Scraper | 4 | Admin |
| Admin: Segmentation | 4 | Admin |
| Admin: User Management | 7 | Admin |
| Static Assets | 2 | No |
| Diagnostic | 1 | Yes |
| **Total** | **67** | |

## Authentication Model

- **Session**: HTTP-only cookies, 7-day expiry
- **Password**: Client-side SHA-256 → server-side Argon2
- **Username Cookie**: Non-HTTP-only for navbar display
- **HTMX Detection**: `HX-Request` header for partial responses

---

## Authentication Routes

Public routes for login, registration, and guest access.

### `GET /login`
Display login page.

**Response:** HTML
**Auth:** None

### `POST /login`
Process login credentials.

| Field | Type | Required | Description |
|-------|------|----------|-------------|
| `username` | String | Yes | User's login name |
| `password_hash` | String | Yes | SHA-256 hash from client |

**Response:** HTML (redirect to `/` on success, login page with error on failure)
**Auth:** None

### `GET /register`
Display registration page.

**Response:** HTML
**Auth:** None

### `POST /register`
Create new user account.

| Field | Type | Required | Description |
|-------|------|----------|-------------|
| `username` | String | Yes | 3-32 chars, alphanumeric/underscore |
| `password_hash` | String | Yes | SHA-256 hash (64 chars) |

**Response:** HTML (redirect to `/` on success)
**Auth:** None

### `GET /guest`
Display guest login page.

**Response:** HTML
**Auth:** None

### `POST /guest`
Create and login as guest account.

| Field | Type | Required | Description |
|-------|------|----------|-------------|
| `nickname` | String | No | Optional display name (max 20 chars) |

**Response:** HTML (redirect to `/` on success)
**Auth:** None

### `POST /logout`
Log out and clear session.

**Response:** Redirect to `/login`
**Auth:** None (clears session if present)

---

## Reference & Guide Routes

Public reference pages accessible without login.

### `GET /guide`
Display learning guide with navigation.

**Response:** HTML
**Auth:** None

### `GET /reference`
Display reference index page.

**Response:** HTML
**Auth:** None

### `GET /reference/basics`
Display Hangul basics reference.

**Response:** HTML
**Auth:** None

### `GET /reference/tier1`
Display Tier 1 characters (basic consonants/vowels).

**Response:** HTML
**Auth:** None

### `GET /reference/tier2`
Display Tier 2 characters (Y-vowels, special ieung).

**Response:** HTML
**Auth:** None

### `GET /reference/tier3`
Display Tier 3 characters (aspirated/tense consonants).

**Response:** HTML
**Auth:** None

### `GET /reference/tier4`
Display Tier 4 characters (compound vowels).

**Response:** HTML
**Auth:** None

### `GET /pronunciation`
Display pronunciation matrix with audio playback.

**Response:** HTML
**Auth:** None

---

## Home

### `GET /`
Home page showing due cards, progress stats, and next review countdown.

**Response:** HTML with:
- `due_count` - Cards due for review
- `unreviewed_count` - New cards (accelerated mode)
- `total_cards` / `cards_learned`
- `next_review` - Countdown to next due card
- `accelerated_mode` / `unlocked_tier`

**Auth:** Required

---

## Study Modes

### Interactive Mode

Interactive study with typed/selected answers and hint system.

#### `GET /study`
Start interactive study session.

**Query Parameters:**

| Param | Type | Description |
|-------|------|-------------|
| `focus_tier` | u8? | Optional tier to focus on (1-4) |

**Response:** HTML
**Auth:** Required

#### `POST /validate-answer`
Validate user's typed answer.

| Field | Type | Required | Description |
|-------|------|----------|-------------|
| `card_id` | i64 | Yes | Card being answered |
| `answer` | String | Yes | User's input |
| `hints_used` | u8 | Yes | Number of hints used |
| `session_id` | String | No | Session tracking |
| `input_method` | String | No | "typed" or "selected" |

**Response:** HTML (feedback with correct/incorrect state)
**Auth:** Required

#### `POST /review`
Submit card review with SRS calculation.

| Field | Type | Required | Description |
|-------|------|----------|-------------|
| `card_id` | i64 | Yes | Card reviewed |
| `quality` | u8 | Yes | Rating (0=Again, 2=Hard, 4=Good, 5=Easy) |
| `session_id` | String | No | Session tracking |

**Response:** HTML (next card or completion screen)
**Auth:** Required
**Side Effects:** Updates FSRS/SM-2 state, character stats, review logs

#### `POST /next-card`
Get next card in study session.

| Field | Type | Required | Description |
|-------|------|----------|-------------|
| `card_id` | i64 | Yes | Previous card (for weighted selection) |
| `session_id` | String | No | Session tracking |

**Response:** HTML
**Auth:** Required

### Classic Mode

Traditional flip-card with manual rating.

#### `GET /study-classic`
Start classic flip-card study mode.

**Response:** HTML
**Auth:** Required

#### `POST /review-classic`
Submit review for classic mode.

| Field | Type | Required | Description |
|-------|------|----------|-------------|
| `card_id` | i64 | Yes | Card reviewed |
| `quality` | u8 | Yes | Rating (0-5) |
| `session_id` | String | No | Session tracking |

**Response:** HTML
**Auth:** Required

### Practice Mode

Review without affecting SRS progression.

#### `GET /practice`
Start practice mode.

**Query Parameters:**

| Param | Type | Default | Description |
|-------|------|---------|-------------|
| `mode` | String | "interactive" | "flip" or "interactive" |
| `track` | bool | true | Whether to track progress |

**Response:** HTML
**Auth:** Required

#### `POST /practice-next`
Get next practice card.

| Field | Type | Required | Description |
|-------|------|----------|-------------|
| `card_id` | i64 | Yes | Previous card |
| `track_progress` | bool | Yes | Track stats |

**Response:** HTML
**Auth:** Required

#### `POST /practice-validate`
Validate answer in practice mode.

| Field | Type | Required | Description |
|-------|------|----------|-------------|
| `card_id` | i64 | Yes | Card being answered |
| `answer` | String | Yes | User's input |
| `track_progress` | bool | Yes | Track stats |
| `input_method` | String | No | Input method used |

**Response:** HTML
**Auth:** Required

---

## Listening Practice

Audio recognition exercises organized by tier.

### `GET /listen`
Display tier selection for listening practice.

**Response:** HTML (tiers with syllable counts)
**Auth:** Required

### `GET /listen/start`
Start listening practice for a tier.

**Query Parameters:**

| Param | Type | Default | Description |
|-------|------|---------|-------------|
| `tier` | u8 | - | Tier to practice (1-4) |
| `hard_mode` | bool | false | Enable hard mode |

**Response:** HTML
**Auth:** Required

### `POST /listen/answer`
Submit answer for listening quiz.

| Field | Type | Required | Description |
|-------|------|----------|-------------|
| `tier` | u8 | Yes | Current tier |
| `answer` | String | Yes | User's answer |
| `correct_syllable` | String | Yes | Expected answer |
| `correct` | u32 | Yes | Correct count so far |
| `total` | u32 | Yes | Total attempts so far |
| `hard_mode` | bool | Yes | Hard mode enabled |

**Response:** HTML
**Auth:** Required

### `POST /listen/answer-htmx`
HTMX-compatible listening answer submission (same params as above).

**Response:** HTML (HTMX partial)
**Auth:** Required

### `GET /listen/skip`
Skip current syllable.

**Query Parameters:**

| Param | Type | Description |
|-------|------|-------------|
| `tier` | u8 | Current tier |
| `correct` | u32 | Correct count |
| `total` | u32 | Total attempts |
| `hard_mode` | bool | Hard mode state |

**Response:** HTML (next syllable)
**Auth:** Required

---

## Progress & Library

### `GET /progress`
Display learning progress and statistics.

**Response:** HTML with:
- Tier progression percentages
- Character-level stats (lifetime, 7d, 1d)
- Problem cards (high confusion rate)

**Auth:** Required

### `POST /unlock-tier`
Manually unlock next tier (when prerequisites met).

**Response:** Redirect to `/progress`
**Auth:** Required

### `GET /library`
Display library landing page.

**Response:** HTML (sections: Characters, Vocabulary)
**Auth:** Required

### `GET /library/characters`
Display all unlocked characters by tier.

**Response:** HTML
**Auth:** Required

### `GET /library/vocabulary`
Display vocabulary entries by lesson.

**Response:** HTML (with usage examples, romanization)
**Auth:** Required

---

## Settings

### `GET /settings`
Display settings page.

**Response:** HTML with:
- Learning preferences
- Tier configuration
- Export/import options
- Admin sections (if admin)

**Auth:** Required

### `POST /settings`
Update user learning preferences.

| Field | Type | Required | Description |
|-------|------|----------|-------------|
| `all_tiers_unlocked` | String | No | Enable accelerated mode |
| `tier_1` ... `tier_4` | String | No | Enable/disable tiers |
| `desired_retention` | u8 | No | FSRS target retention (%) |
| `focus_tier` | String | No | "none" or "1"-"4" |

**Response:** Redirect to `/settings`
**Auth:** Required

### `GET /settings/export`
Export user learning data as JSON.

**Response:** JSON file (`application/json`)
**Auth:** Required

### `POST /settings/import`
Import learning data from JSON file.

**Request:** Multipart form with file upload
**Response:** Redirect to `/settings`
**Auth:** Required

---

## Content Packs

### `POST /settings/pack/{pack_id}/enable`
Enable a content pack for the user.

**Path Parameter:** `pack_id` (String)
**Response:** Redirect to `/settings`
**Auth:** Required

### `POST /settings/pack/{pack_id}/disable`
Disable a content pack for the user.

**Path Parameter:** `pack_id` (String)
**Response:** Redirect to `/settings`
**Auth:** Required

### `POST /settings/pack/{pack_id}/make-public`
Make pack available to all users. **Admin only.**

**Path Parameter:** `pack_id` (String)
**Response:** Redirect
**Auth:** Admin

### `POST /settings/pack/permission/add`
Restrict pack to specific group. **Admin only.**

| Field | Type | Required | Description |
|-------|------|----------|-------------|
| `pack_id` | String | Yes | Pack to restrict |
| `group_id` | String | Yes | Group to allow |

**Response:** HTML (HTMX) or Redirect
**Auth:** Admin

### `POST /settings/pack/permission/remove`
Remove pack group restriction. **Admin only.**

| Field | Type | Required | Description |
|-------|------|----------|-------------|
| `pack_id` | String | Yes | Pack ID |
| `group_id` | String | Yes | Group to remove |

**Response:** HTML (HTMX) or Redirect
**Auth:** Admin

### `POST /settings/pack/user-permission/add`
Grant user direct pack access. **Admin only.**

| Field | Type | Required | Description |
|-------|------|----------|-------------|
| `pack_id` | String | Yes | Pack ID |
| `user_id` | i64 | Yes | User to grant access |

**Response:** HTML (HTMX) or Redirect
**Auth:** Admin

### `POST /settings/pack/user-permission/remove`
Revoke user direct pack access. **Admin only.**

| Field | Type | Required | Description |
|-------|------|----------|-------------|
| `pack_id` | String | Yes | Pack ID |
| `user_id` | i64 | Yes | User to revoke |

**Response:** HTML (HTMX) or Redirect
**Auth:** Admin

---

## Admin: Scraper Operations

Audio scraping from howtostudykorean.com. **Admin only.**

### `POST /settings/scrape`
Scrape all lessons (1, 2, 3).

**Response:** Redirect to `/settings`
**Auth:** Admin
**Side Effects:** Runs Python scraper

### `POST /settings/scrape/{lesson}`
Scrape specific lesson.

**Path Parameter:** `lesson` ("1", "2", or "3")
**Response:** Redirect to `/settings`
**Auth:** Admin

### `POST /settings/delete-scraped`
Delete all scraped content.

**Response:** Redirect to `/settings`
**Auth:** Admin

### `POST /settings/delete-scraped/{lesson}`
Delete specific lesson's scraped content.

**Path Parameter:** `lesson` ("1", "2", or "3")
**Response:** Redirect to `/settings`
**Auth:** Admin

---

## Admin: Audio Segmentation

Fine-tune audio segment boundaries. **Admin only.**

### `POST /settings/segment`
Re-segment all syllables with custom padding.

| Field | Type | Default | Description |
|-------|------|---------|-------------|
| `padding` | u32 | 75 | Padding in milliseconds |

**Response:** HTML (HTMX with status)
**Auth:** Admin

### `POST /settings/segment-row`
Re-segment specific consonant/vowel row.

| Field | Type | Default | Description |
|-------|------|---------|-------------|
| `lesson` | String | - | Lesson number |
| `row` | String | - | Romanization (e.g., "ga") |
| `min_silence` | i32 | 200 | Min silence detection (ms) |
| `threshold` | i32 | -40 | Silence threshold (dBFS) |
| `padding` | i32 | 75 | Padding (ms) |
| `skip_first` | i32 | 0 | Skip first N segments |
| `skip_last` | i32 | 0 | Skip last N segments |

**Response:** HTML (HTMX with updated row)
**Auth:** Admin

### `POST /settings/segment-manual`
Apply manual segment timestamps.

| Field | Type | Required | Description |
|-------|------|----------|-------------|
| `lesson` | String | Yes | Lesson number |
| `syllable` | String | Yes | Korean character |
| `romanization` | String | Yes | Filename |
| `row` | String | Yes | Row for UI refresh |
| `start_ms` | i32 | Yes | Start time (ms) |
| `end_ms` | i32 | Yes | End time (ms) |

**Response:** HTML (HTMX with status)
**Auth:** Admin

### `POST /settings/segment-reset`
Reset manual timestamps to baseline.

| Field | Type | Required | Description |
|-------|------|----------|-------------|
| `lesson` | String | Yes | Lesson number |
| `syllable` | String | Yes | Korean character |
| `romanization` | String | Yes | Filename |
| `row` | String | Yes | Row for UI refresh |

**Response:** HTML (HTMX with status)
**Auth:** Admin

---

## Admin: Learning State

Manage card learning states. **Admin only.**

### `POST /settings/make-all-due`
Reset all cards to due status.

**Response:** Redirect to `/settings`
**Auth:** Admin

### `POST /settings/graduate-tier/{tier}`
Mark all tier cards as learned.

**Path Parameter:** `tier` ("1", "2", "3", or "4")
**Response:** Redirect to `/settings`
**Auth:** Admin

### `POST /settings/restore-tier/{tier}`
Reset graduated tier back to learning.

**Path Parameter:** `tier` ("1", "2", "3", or "4")
**Response:** Redirect to `/settings`
**Auth:** Admin

---

## Admin: Guest Management

Manage guest accounts. **Admin only.**

### `POST /settings/cleanup-guests`
Clean up expired guest accounts.

**Response:** Redirect to `/settings`
**Auth:** Admin

### `POST /settings/delete-all-guests`
Delete all guest accounts.

**Response:** Redirect to `/settings`
**Auth:** Admin

---

## Admin: User & Group Management

Manage users, roles, and groups. **Admin only.**

### `POST /settings/user/role`
Change user role.

| Field | Type | Required | Description |
|-------|------|----------|-------------|
| `user_id` | i64 | Yes | User to modify |
| `role` | String | Yes | "user" or "admin" |

**Response:** Redirect to `/settings`
**Auth:** Admin

### `POST /settings/group/create`
Create new user group.

| Field | Type | Required | Description |
|-------|------|----------|-------------|
| `id` | String | Yes | Unique group ID |
| `name` | String | Yes | Display name |
| `description` | String | No | Group description |

**Response:** HTML (HTMX) or Redirect
**Auth:** Admin

### `DELETE /settings/group/{group_id}`
Delete user group.

**Path Parameter:** `group_id` (String)
**Response:** HTML (HTMX) or Redirect
**Auth:** Admin

### `POST /settings/group/add-member`
Add user to group.

| Field | Type | Required | Description |
|-------|------|----------|-------------|
| `user_id` | i64 | Yes | User to add |
| `group_id` | String | Yes | Target group |

**Response:** HTML (HTMX) or Redirect
**Auth:** Admin

### `POST /settings/group/remove-member`
Remove user from group.

| Field | Type | Required | Description |
|-------|------|----------|-------------|
| `user_id` | i64 | Yes | User to remove |
| `group_id` | String | Yes | Source group |

**Response:** HTML (HTMX) or Redirect
**Auth:** Admin

---

## Diagnostic

### `POST /diagnostic`
Log diagnostic information for debugging card display issues.

| Field | Type | Required | Description |
|-------|------|----------|-------------|
| `card_id` | i64 | Yes | Card ID |
| `displayed_front` | String | Yes | What was shown |
| `displayed_answer` | String | Yes | Expected answer shown |

**Response:** HTML (status message)
**Auth:** Required
**Side Effects:** Writes diagnostic report to file

---

## Static Assets

### `GET /audio/scraped/**`
Serve scraped audio files (MP3).

**Path:** Files under `static/scraped/`
**Response:** `audio/mpeg`
**Auth:** None

### `GET /static/**`
Serve static assets (CSS, JS, images).

**Path:** Files under `static/`
**Response:** Various MIME types
**Auth:** None

---

## Response Patterns

### HTML Pages
Most endpoints return full HTML pages rendered by Askama templates.

### HTMX Partials
Endpoints supporting HTMX return partial HTML when `HX-Request` header is present:
- Segmentation controls (`/settings/segment-*`)
- Group management (`/settings/group/*`)
- Pack permissions (`/settings/pack/permission/*`)

### Redirects
Form submissions typically redirect on success:
- Settings updates → `/settings`
- Progress actions → `/progress`
- Login/Register → `/`

### JSON
Only `/settings/export` returns JSON (for data export).
