# Card Selection Algorithm

This document describes how cards are selected for study sessions.

## Overview

Card selection uses three mechanisms:
1. **Reinforcement Queue** - Failed cards re-shown within 3-5 cards
2. **Weighted Random Selection** - Cards chosen based on performance factors
3. **Sibling Exclusion** - Prevents consecutive similar cards

## Selection Flow

```
┌─────────────────────┐
│  Request Next Card  │
└──────────┬──────────┘
           │
           ▼
┌─────────────────────┐    Yes    ┌───────────────────┐
│ Reinforcement Queue ├──────────►│ Return Reinforce  │
│ due? (≥3 cards)     │           │ Card (FIFO)       │
└──────────┬──────────┘           └───────────────────┘
           │ No
           ▼
┌─────────────────────┐
│ Calculate Weights   │
│ for All Due Cards   │
└──────────┬──────────┘
           │
           ▼
┌─────────────────────┐
│ Weighted Random     │
│ Select (exclude     │
│ last card shown)    │
└──────────┬──────────┘
           │
           ▼
┌─────────────────────┐
│ Sibling Exclusion   │
│ (SQL-level filter)  │
└──────────┬──────────┘
           │
           ▼
┌─────────────────────┐
│ Return Selected     │
│ Card                │
└─────────────────────┘
```

---

## Reinforcement Queue

Failed cards are re-shown after 3-5 regular cards to reinforce learning.

### Behavior

| Event | Action |
|-------|--------|
| Card answered incorrectly | Add to reinforcement queue |
| Card answered correctly | Remove from queue (if present) |
| 3+ cards shown since last reinforce | Pop next from queue |

### Implementation

```rust
pub struct StudySession {
  pub reinforcement_queue: VecDeque<i64>,  // Failed card IDs
  pub cards_since_reinforce: u32,          // Counter
  pub last_card_id: Option<i64>,           // Prevent immediate repeat
}
```

- **Queue Order**: FIFO (first failed = first re-shown)
- **No Duplicates**: Same card won't appear twice in queue
- **Threshold**: Must show 3 regular cards before reinforcement

---

## Weight Calculation

Each card's selection probability is based on 4 factors:

### Factor 1: Success Rate

Lower success rate = higher weight.

| Success Rate | Multiplier |
|--------------|------------|
| 100% | 1.0x |
| 50% | 1.5x |
| 0% | 2.0x |
| New card | 1.5x (assumes 50%) |

**Formula:** `weight *= 2.0 - success_rate`

### Factor 2: Recent Failure Recency

Recently failed cards get significant boosts.

| Time Since Failure | Multiplier |
|-------------------|------------|
| < 5 minutes | 10.0x |
| 5-30 minutes | 3.0x |
| 30-60 minutes | 1.5x |
| > 60 minutes | 1.0x |

### Factor 3: Review Count

New or barely reviewed cards get priority.

| Total Reviews | Multiplier |
|---------------|------------|
| 0 (never reviewed) | 2.0x |
| 1-2 | 1.5x |
| 3-4 | 1.2x |
| 5+ | 1.0x |

### Factor 4: Time Since Last Review

Older cards get slightly higher priority.

| Hours Since Last | Multiplier |
|------------------|------------|
| 0-1 | 1.0x-1.1x |
| 2-5 | 1.2x-1.5x |
| 10+ | 2.0x (capped) |

**Formula:** `weight *= 1.0 + (hours * 0.1).min(1.0)`

### Combined Example

A card with:
- 20% success rate → 1.8x
- Failed 3 minutes ago → 10.0x
- 2 total reviews → 1.5x
- Last review 1 hour ago → 1.1x

**Total weight:** 1.8 × 10.0 × 1.5 × 1.1 = **29.7**

---

## Sibling Exclusion

Prevents consecutive cards with similar content.

### Exclusion Rules

When selecting the next card, SQL filters exclude:

```sql
WHERE cd.id != <last_card_id>           -- Not the same card
  AND cd.main_answer != <last_front>    -- Not the same answer
  AND cd.front NOT LIKE '%' || <last_front> || '%'  -- Answer not in front
```

### Purpose

| Scenario | Without Exclusion | With Exclusion |
|----------|-------------------|----------------|
| "ㄱ → ga" followed by | Could show "ga → ㄱ" | Shows different card |
| "가 → ga" followed by | Could show "까 → kka (contains 가)" | Different character |

### Functions Using Sibling Exclusion

| Function | File |
|----------|------|
| `get_due_cards()` | `src/db/cards.rs:84` |
| `get_due_cards_interleaved()` | `src/db/cards.rs:237` |
| `get_practice_cards()` | `src/db/cards.rs:315` |
| `get_unreviewed_today()` | `src/db/cards.rs:451` |

---

## Interleaving

When `use_interleaving` setting is enabled, cards are grouped by type before randomization.

### SQL Pattern

```sql
ORDER BY cd.card_type, RANDOM()
```

### Effect

Instead of random order across all types:
```
Consonant → Vowel → Vowel → Consonant → Vowel
```

Interleaving produces:
```
Consonant → Vowel → Consonant → Vowel → Consonant
```

### Purpose

Interleaving (variable practice) improves learning by:
- Forcing context switching between card types
- Preventing "type fatigue" (all consonants in a row)
- Matching research on optimal spaced practice

---

## Configuration

| Setting | Default | Effect |
|---------|---------|--------|
| `use_interleaving` | `true` | Mix card types in study order |
| `focus_tier` | none | Restrict to single tier |
| `enabled_tiers` | `1,2,3,4` | Which tiers to include |

---

## Source Files

| File | Purpose |
|------|---------|
| `src/srs/card_selector.rs` | Weight calculation, reinforcement queue |
| `src/db/cards.rs` | Card queries with sibling exclusion |
| `src/handlers/study/interactive.rs` | Session management, card selection calls |
