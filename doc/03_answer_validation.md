# Answer Validation

This document describes the answer validation logic for Hangul learning.

## Overview

The validation system supports:
- Romanization variants (e.g., "g / k" accepts both)
- Phonetic modifiers (e.g., "jj (tense)")
- Typo tolerance via Levenshtein distance
- Korean character matching

## Matching Flow

```
User Input
    │
    ▼
┌─────────────────────────────────────┐
│ Normalize                           │
│ - Lowercase                         │
│ - Trim whitespace                   │
│ - Remove special chars (keep / )   │
│ - Preserve Korean characters        │
└─────────────────────────────────────┘
    │
    ▼
┌─────────────────────────────────────┐
│ Extract Variants from Correct       │
│ "g / k" → ["g / k", "g", "k", "g/k"]│
└─────────────────────────────────────┘
    │
    ▼
┌─────────────────────────────────────┐    Yes    ┌──────────────┐
│ Exact match with any variant?       ├──────────►│ Correct      │
└─────────────────────────────────────┘           └──────────────┘
    │ No
    ▼
┌─────────────────────────────────────┐
│ Has phonetic modifier?              │
│ (tense, aspirated)                  │
└─────────────────────────────────────┘
    │ Yes                        │ No
    ▼                            ▼
┌──────────────────┐    ┌─────────────────────────┐
│ Check core +     │    │ Levenshtein distance    │
│ modifier match   │    │ within tolerance?       │
└──────────────────┘    └─────────────────────────┘
    │                            │
    ▼                            ▼
Result                    Result
```

---

## Variant Extraction

Answers with "/" are split into multiple acceptable variants.

### Examples

| Main Answer | Variants Generated |
|-------------|-------------------|
| `g / k` | `"g / k"`, `"g"`, `"k"`, `"g/k"` |
| `ng` | `"ng"` |
| `ya` | `"ya"` |
| `eo / o` | `"eo / o"`, `"eo"`, `"o"`, `"eo/o"` |

### Logic

```rust
fn extract_variants(main_answer: &str) -> Vec<String> {
    // 1. Add normalized original
    // 2. If contains "/", split and add each part
    // 3. Add joined version without spaces
}
```

---

## Phonetic Modifiers

Cards with phonetic descriptors like "(tense)" or "(aspirated)" require both the core letter and modifier.

### Parsing

```
"jj (tense)"  → core="jj", modifier="tense"
"ch aspirated" → core="ch", modifier="aspirated"
"ya"           → core="ya", modifier=None
```

### Matching Rules

| Input | Correct | Result | Reason |
|-------|---------|--------|--------|
| `jj tense` | `jj (tense)` | Correct | Letter + modifier match |
| `jj tenss` | `jj (tense)` | CloseEnough | 1-edit typo in modifier |
| `jj tenes` | `jj (tense)` | Incorrect | 2+ edits in modifier |
| `jj` | `jj (tense)` | Incorrect | Missing modifier |
| `jj aspirated` | `jj (tense)` | Incorrect | Wrong modifier |
| `ch tense` | `jj (tense)` | Incorrect | Wrong core letter |

### Key Rule

**Core letter must match exactly** - no typo tolerance for the consonant/vowel itself.

---

## Distance Thresholds

For non-modifier answers, Levenshtein distance determines tolerance.

### Thresholds by Length

| Answer Length | Max Distance | Example |
|---------------|--------------|---------|
| 1 char | 0 (exact) | `"g"` must be exact |
| 2-4 chars | 1 | `"ya"` accepts `"yo"` |
| 5+ chars | 1 | Same tolerance |

### Result Mapping

| Distance | Result |
|----------|--------|
| 0 | Correct |
| 1 (within threshold) | CloseEnough |
| 2+ | Incorrect |

### Examples

| Input | Correct | Distance | Result |
|-------|---------|----------|--------|
| `ya` | `ya` | 0 | Correct |
| `yo` | `ya` | 1 | CloseEnough |
| `xyz` | `ya` | 3 | Incorrect |
| `g` | `k` | 1 | Incorrect (single char must be exact) |

---

## Korean Characters

Korean jamo and syllables are preserved and matched exactly.

### Normalization

```rust
normalize_answer("ㄱ") → "ㄱ"    // preserved
normalize_answer("가") → "가"    // preserved
normalize_answer("  가  ") → "가" // whitespace trimmed
```

### Matching

| Input | Correct | Result |
|-------|---------|--------|
| `ㄱ` | `ㄱ` | Correct |
| `ㄴ` | `ㄱ` | Incorrect |
| `가` | `가` | Correct |
| `ㄲ` | `ㄱ` | Incorrect (different character) |

**No typo tolerance for Korean** - characters must match exactly.

---

## Quality Mapping

Answer results map to SRS quality ratings:

| Result | Hint Used | Quality | SRS Effect |
|--------|-----------|---------|------------|
| Correct | No | 4 (Good) | Normal interval increase |
| Correct | Yes | 2 (Hard) | Smaller interval increase |
| CloseEnough | Any | 2 (Hard) | Smaller interval increase |
| Incorrect | Any | 0 (Again) | Reset to learning phase |

### Code

```rust
pub fn to_quality(&self, used_hint: bool) -> u8 {
    match (self, used_hint) {
        (Correct, false) => 4,     // Good
        (Correct, true) => 2,      // Hard
        (CloseEnough, _) => 2,     // Hard
        (Incorrect, _) => 0,       // Again
    }
}
```

---

## Hint System

Progressive hints reveal the answer gradually.

### Levels

| Level | Example for "g / k" |
|-------|---------------------|
| 1 | `g____ (5 letters)` |
| 2 | Description or `g/__` |
| 3 | `g / k` (full answer) |

### Implementation

```rust
pub struct HintGenerator {
    answer: String,
    description: Option<String>,
}

impl HintGenerator {
    pub fn hint_level_1(&self) -> String;  // First char + length
    pub fn hint_level_2(&self) -> String;  // Description or more chars
    pub fn hint_final(&self) -> String;    // Full answer
}
```

---

## Edge Cases

### Empty Input

```rust
validate_answer("", "g / k") → Incorrect
```

### Whitespace Handling

```rust
validate_answer("  g  ", "g / k") → Correct  // trimmed
validate_answer("g/k", "g / k") → Correct   // slash variants
validate_answer("g  /  k", "g / k") → Correct // normalized
```

### Case Insensitivity

```rust
validate_answer("G", "g / k") → Correct
validate_answer("YA", "ya") → Correct
```

---

## Source File

| File | Purpose |
|------|---------|
| `src/validation.rs` | All validation logic, hint generation |
