# Learning Mode Finite State Automaton (FSA)

This document describes the state machine that controls how cards are presented during study sessions.

## Overview

The app has two main learning modes:
- **Normal Mode**: Progressive unlock system with spaced repetition
- **Accelerated Mode**: All enabled tiers available, study all cards daily

## State Definitions

### Card States
- **Due**: Cards with `next_review <= now` (scheduled by FSRS/SM-2)
- **Unreviewed Today**: Cards not yet reviewed in today's session (tracked via `review_logs`)
- **Reviewed Today**: Cards that have been reviewed at least once today

### Mode Detection
- `accelerated_mode = all_tiers_unlocked` setting in database
- When `true`, accelerated FSA is used
- When `false`, normal FSA is used

## Normal Mode FSA

```
┌─────────────────────────────────────────────────────────┐
│                     NORMAL MODE                          │
├─────────────────────────────────────────────────────────┤
│                                                          │
│   ┌──────────┐                                          │
│   │  START   │                                          │
│   └────┬─────┘                                          │
│        │                                                 │
│        ▼                                                 │
│   ┌──────────┐    Yes    ┌─────────────────┐           │
│   │ Due cards ├─────────►│  Show Due Card  │           │
│   │  exist?   │          └────────┬────────┘           │
│   └────┬─────┘                    │                     │
│        │ No                       │ Review submitted    │
│        ▼                          ▼                     │
│   ┌──────────────┐         ┌─────────────┐             │
│   │ Practice Mode │◄────────┤ More due?   │             │
│   │   (optional)  │   No    └──────┬──────┘             │
│   └──────────────┘                 │ Yes                │
│                                    └───────────────┐    │
│                                                    │    │
│                              (loop back to Show)◄──┘    │
│                                                          │
└─────────────────────────────────────────────────────────┘
```

### Transitions
1. **START → Check Due**: Initial state
2. **Check Due → Show Card**: If `due_count > 0`
3. **Check Due → Practice Mode**: If `due_count == 0`
4. **Show Card → Check Due**: After review submitted
5. **Practice Mode**: End state (optional extra practice)

## Accelerated Mode FSA

```
┌─────────────────────────────────────────────────────────┐
│                   ACCELERATED MODE                       │
├─────────────────────────────────────────────────────────┤
│                                                          │
│   ┌──────────┐                                          │
│   │  START   │                                          │
│   └────┬─────┘                                          │
│        │                                                 │
│        ▼                                                 │
│   ┌──────────┐    Yes    ┌─────────────────┐           │
│   │ Due cards ├─────────►│  Show Due Card  │           │
│   │  exist?   │          └────────┬────────┘           │
│   └────┬─────┘                    │                     │
│        │ No                       │ Review submitted    │
│        ▼                          ▼                     │
│   ┌────────────┐    Yes   ┌─────────────┐              │
│   │ Unreviewed ├─────────►│Show Unrev.  │              │
│   │   today?   │          │   Card      │              │
│   └────┬───────┘          └──────┬──────┘              │
│        │ No                      │                      │
│        ▼                         │ Review submitted     │
│   ┌──────────────┐               ▼                      │
│   │ Practice Mode │◄─────── Check states again         │
│   │  (all done!)  │                                     │
│   └──────────────┘                                      │
│                                                          │
└─────────────────────────────────────────────────────────┘
```

### Transitions
1. **START → Check Due**: Initial state
2. **Check Due → Show Due Card**: If `due_count > 0`
3. **Check Due → Check Unreviewed**: If `due_count == 0`
4. **Check Unreviewed → Show Unreviewed Card**: If `unreviewed_today_count > 0`
5. **Check Unreviewed → Practice Mode**: If `unreviewed_today_count == 0`
6. **Show Card → Check Due**: After review (due cards take priority)
7. **Practice Mode**: End state (all cards reviewed today)

## Implementation

### Key Functions

#### `get_next_study_card()` in `src/handlers/study.rs`
```rust
fn get_next_study_card(conn, exclude_sibling_of) -> Option<Card> {
    // Step 1: Try due cards (both modes)
    if let Some(card) = get_due_cards(...) {
        return Some(card);
    }

    // Step 2: In accelerated mode, try unreviewed today
    if accelerated {
        if let Some(card) = get_unreviewed_today(...) {
            return Some(card);
        }
    }

    // Step 3: No cards - show practice mode
    None
}
```

#### `get_unreviewed_today()` in `src/db/repository.rs`
Returns cards from enabled tiers that haven't been reviewed today:
```sql
SELECT * FROM cards c
WHERE c.tier IN (enabled_tiers)
  AND c.id NOT IN (
    SELECT DISTINCT card_id FROM review_logs
    WHERE date(reviewed_at) = date('now')
  )
```

### Homepage Display

The homepage (`index.html`) shows different information based on mode:

| Mode | Display | Button |
|------|---------|--------|
| Normal, cards due | "{n} Cards due for review" | "Start Studying" |
| Normal, none due | "Next review in {time}" | "Practice Anyway" |
| Accelerated, cards available | "{n} Cards to study today" | "Start Studying" |
| Accelerated, all done | "All done for today!" | "Practice Mode" |

## Daily Reset

In accelerated mode, the "unreviewed today" list resets at midnight (local time via SQLite's `date('now')`). This means:
- Each day, all cards in enabled tiers become "unreviewed"
- Users can study their entire deck daily if desired
- The spaced repetition schedule still applies (due cards shown first)

## Configuration

Settings in `settings` table:
- `all_tiers_unlocked`: `true` = accelerated mode, `false` = normal mode
- `enabled_tiers`: Comma-separated list (e.g., "1,2,3,4")
- `use_interleaving`: Whether to mix card types within a session
