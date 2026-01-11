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

#### Card Selection in `src/handlers/study/interactive.rs`
```rust
// Step 1: Check reinforcement queue (failed cards)
if let Some(reinforce_id) = session.pop_reinforcement() {
    return Some(reinforce_id);
}

// Step 2: Try due cards (both modes)
if let Some(card) = get_due_cards(...) {
    return Some(card);
}

// Step 3: In accelerated mode, try unreviewed today
if accelerated {
    if let Some(card) = get_unreviewed_today(...) {
        return Some(card);
    }
}

// Step 4: No cards - show practice mode
None
```

#### `get_unreviewed_today()` in `src/db/cards.rs`
Returns cards from enabled tiers that haven't been reviewed today:
```sql
SELECT * FROM card_definitions cd
LEFT JOIN card_progress cp ON cd.id = cp.card_id
WHERE cd.tier IN (enabled_tiers)
  AND cd.id NOT IN (
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
- `focus_tier`: If set, restricts study to single tier

---

## Learning Steps (Hybrid System)

Cards use a hybrid system: Anki-style learning steps (0-3) before graduating to FSRS (step 4+).

### Step Progression

```
┌─────────┐    Correct   ┌─────────┐    Correct   ┌─────────┐    Correct   ┌─────────┐    Correct   ┌────────────┐
│ Step 0  │────────────►│ Step 1  │────────────►│ Step 2  │────────────►│ Step 3  │────────────►│ Graduated  │
│ (1 min) │             │ (10 min)│             │ (1 hr)  │             │ (4 hr)  │             │ (FSRS)     │
└─────────┘             └─────────┘             └─────────┘             └─────────┘             └────────────┘
     ▲                       │                       │                       │                       │
     │                       │                       │                       │                       │
     └───────────────────────┴───────────────────────┴───────────────────────┴───────────────────────┘
                                              Wrong = Reset to Step 0
```

### Normal Mode Intervals

| Step | Interval |
|------|----------|
| 0 | 1 minute |
| 1 | 10 minutes |
| 2 | 1 hour |
| 3 | 4 hours |
| 4+ | FSRS calculates (~1+ day) |

### Focus Mode Intervals

Focus mode uses faster learning steps for intensive practice:

| Step | Interval |
|------|----------|
| 0 | 1 minute |
| 1 | 5 minutes |
| 2 | 15 minutes |
| 3 | 30 minutes |
| 4+ | FSRS calculates |

**Total time to graduate:**
- Normal mode: ~5 hours
- Focus mode: ~50 minutes

---

## Relearning Phase

When a graduated card (step 4+) is answered incorrectly, it returns to the learning phase.

```
┌────────────────────────────────────────────────────────────────────────────┐
│                                                                             │
│   ┌────────────┐    Wrong    ┌─────────┐                                  │
│   │ Graduated  │────────────►│ Step 0  │ (Restart learning steps)         │
│   │ (FSRS)     │             │ (1 min) │                                  │
│   └────────────┘             └─────────┘                                  │
│         ▲                         │                                        │
│         │                         │ Correct × 4                            │
│         └─────────────────────────┘                                        │
│              Re-graduate to FSRS                                           │
│                                                                             │
└────────────────────────────────────────────────────────────────────────────┘
```

### Key Behavior

- **Failure resets to step 0** - even for graduated cards
- **Must complete all 4 learning steps again**
- **Re-graduation**: After step 3 is correct, card returns to FSRS
- **FSRS state preserved**: Stability/difficulty values carry forward

---

## Source Files

| File | Purpose |
|------|---------|
| `src/handlers/study/interactive.rs` | Study session state machine |
| `src/handlers/study/mod.rs` | Card selection coordination |
| `src/db/cards.rs` | Card queries (due, unreviewed, practice) |
| `src/srs/fsrs_scheduler.rs` | Learning steps, FSRS graduation |
| `src/srs/card_selector.rs` | Weighted selection, reinforcement queue |
