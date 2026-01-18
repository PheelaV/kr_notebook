# Answer Validation

This document describes the answer validation logic for kr_notebook.

## Overview

The validation system supports:
- **Structured grammar**: `[variants]`, `word(s)` suffixes, `(info)`, `<disambiguation>`
- **Automatic normalization**: British/American spellings, contractions
- **Typo tolerance**: Levenshtein distance with length-based thresholds
- **Permutation matching**: Comma-separated synonyms in any order
- **Partial credit**: Disambiguation contexts reward partial knowledge

See also: `doc/12_content_authoring.md` for pack authoring guide.

---

## Answer Grammar

The `main_answer` field supports structured grammar for flexible validation.

### Grammar Elements

| Syntax | Type | Validation | Display |
|--------|------|------------|---------|
| `[a, b, c]` | **Variants** | Any item correct | `≈[a, b, c]` |
| `word(s)` | **Suffix** | With/without suffix | `word≈(s)` |
| `(info)` | **Info** | Ignored | `ℹ(info)` |
| `<context>` | **Disambiguation** | Required for full credit | `△<context>` |
| `a, b` | **Synonyms** | Any order accepted | `a, b` |

### Examples

| Main Answer | Valid Inputs | Invalid |
|-------------|--------------|---------|
| `to be [is, am, are]` | "to be", "is", "am", "are" | "being" |
| `eye(s)` | "eye", "eyes" | "eyees" |
| `that <far>` | "that far" (full), "that" (partial) | "far" |
| `sofa, couch` | "sofa", "couch", "sofa couch", "couch sofa" | "chair" |
| `I, me (formal)` | "I", "me", "I me", "me I" | "formal" |

### Parsing Rules

1. **Brackets `[...]`**: Split by comma, each item is acceptable
2. **Suffix `word(...)`**: No space before paren = optional suffix
3. **Info `word (...)`**: Space before paren = supplementary (ignored)
4. **Angle brackets `<...>`**: Disambiguation required for full credit
5. **Top-level commas**: Create synonyms accepting any order

---

## Matching Flow

```
User Input
    │
    ▼
┌─────────────────────────────────────┐
│ Normalize Input                     │
│ - Lowercase                         │
│ - Trim/collapse whitespace          │
│ - British → American spellings      │
│ - Expand contractions               │
│ - Preserve Korean characters        │
└─────────────────────────────────────┘
    │
    ▼
┌─────────────────────────────────────┐
│ Parse Grammar                       │
│ - Extract [variants]                │
│ - Identify (s) suffixes             │
│ - Strip (info) and <disambiguation> │
│ - Split comma synonyms              │
└─────────────────────────────────────┘
    │
    ▼
┌─────────────────────────────────────┐    Yes    ┌──────────────┐
│ Exact match with any variant?       ├──────────►│ Correct      │
└─────────────────────────────────────┘           └──────────────┘
    │ No
    ▼
┌─────────────────────────────────────┐    Yes    ┌──────────────┐
│ Permutation match (synonyms)?       ├──────────►│ Correct      │
└─────────────────────────────────────┘           └──────────────┘
    │ No
    ▼
┌─────────────────────────────────────┐    Yes    ┌──────────────┐
│ Core match + disambiguation?        ├──────────►│ PartialMatch │
│ (missing <context> only)            │           └──────────────┘
└─────────────────────────────────────┘
    │ No
    ▼
┌─────────────────────────────────────┐    Yes    ┌──────────────┐
│ Levenshtein within tolerance?       ├──────────►│ CloseEnough  │
└─────────────────────────────────────┘           └──────────────┘
    │ No
    ▼
┌──────────────┐
│ Incorrect    │
└──────────────┘
```

---

## Normalization

### Automatic Transformations

All comparisons use normalized forms:

1. **Case**: `"HELLO"` → `"hello"`
2. **Whitespace**: `"  hello   world  "` → `"hello world"`
3. **British/American**: `"colour"` → `"color"`, `"analyse"` → `"analyze"`
4. **Contractions**: `"I'm"` → `"I am"`, `"don't"` → `"do not"`

### Spelling Normalization Table

| British | American |
|---------|----------|
| colour | color |
| favourite | favorite |
| analyse | analyze |
| centre | center |
| travelling | traveling |

### Contraction Expansion Table

| Contraction | Expansion |
|-------------|-----------|
| I'm | I am |
| you're | you are |
| don't | do not |
| can't | cannot |
| won't | will not |
| isn't | is not |
| they're | they are |
| we're | we are |

---

## Typo Tolerance

Levenshtein distance determines typo tolerance for text input.

### Thresholds by Length

| Answer Length | Max Distance | Example |
|---------------|--------------|---------|
| 1 character | 0 (exact) | `"g"` must be exact |
| 2-4 characters | 1 | `"eye"` accepts `"eey"` |
| 5+ characters | 2 | `"school"` accepts `"shcool"` |

### Result Mapping

| Distance | Within Threshold | Result |
|----------|------------------|--------|
| 0 | Yes | Correct |
| 1-2 | Yes | CloseEnough |
| Any | No | Incorrect |

### Examples

| Input | Correct | Distance | Result |
|-------|---------|----------|--------|
| `eye` | `eye` | 0 | Correct |
| `eey` | `eye` | 1 | CloseEnough |
| `eyse` | `eyes` | 1 | CloseEnough |
| `g` | `k` | 1 | Incorrect (single char) |
| `school` | `school` | 0 | Correct |
| `shcool` | `school` | 2 | CloseEnough |

---

## Permutation Matching

Comma-separated synonyms accept any word order.

### How It Works

For `"sofa, couch"`:
1. Extract terms: `["sofa", "couch"]`
2. User input split into words
3. Match if all input words are valid terms

### Examples

| Main Answer | Input | Match? |
|-------------|-------|--------|
| `sofa, couch` | `"sofa"` | Yes (subset) |
| `sofa, couch` | `"couch"` | Yes (subset) |
| `sofa, couch` | `"sofa couch"` | Yes (all terms) |
| `sofa, couch` | `"couch sofa"` | Yes (any order) |
| `sofa, couch` | `"chair"` | No |
| `I, me` | `"me I"` | Yes |

---

## Partial Match

When a card has disambiguation context `<...>`, users get partial credit for the core answer.

### Behavior

| Main Answer | Input | Result | Quality |
|-------------|-------|--------|---------|
| `that <far>` | `"that far"` | Correct | 4 |
| `that <far>` | `"that"` | PartialMatch | 2 |
| `that <far>` | `"far"` | Incorrect | 0 |
| `that <far>` | `"this"` | Incorrect | 0 |

### Retry Mechanic

`PartialMatch` allows retry with a "shake" animation feedback. The user can add the disambiguation for full credit without penalty.

---

## Phonetic Modifiers

Cards with phonetic descriptors like `(tense)` or `(aspirated)` require both core and modifier.

### Parsing

```
"jj (tense)"    → core="jj", modifier="tense"
"ch (aspirated)" → core="ch", modifier="aspirated"
"ya"            → core="ya", modifier=None
```

### Matching Rules

| Input | Correct | Result | Reason |
|-------|---------|--------|--------|
| `jj tense` | `jj (tense)` | Correct | Core + modifier match |
| `jj tenss` | `jj (tense)` | CloseEnough | 1-edit typo in modifier |
| `jj` | `jj (tense)` | Incorrect | Missing modifier |
| `jj aspirated` | `jj (tense)` | Incorrect | Wrong modifier |

**Note**: Core letter must match exactly (no typo tolerance).

---

## Korean Characters

Korean jamo and syllables are preserved and matched exactly.

### Normalization

```rust
normalize_answer("ㄱ") → "ㄱ"     // preserved
normalize_answer("가") → "가"     // preserved
normalize_answer("  가  ") → "가"  // whitespace trimmed
```

### Matching

| Input | Correct | Result |
|-------|---------|--------|
| `ㄱ` | `ㄱ` | Correct |
| `ㄴ` | `ㄱ` | Incorrect |
| `가` | `가` | Correct |
| `ㄲ` | `ㄱ` | Incorrect |

**No typo tolerance for Korean** - characters must match exactly.

---

## Quality Mapping

Answer results map to SRS quality ratings:

| Result | Hint Used | Quality | SRS Effect |
|--------|-----------|---------|------------|
| Correct | No | 4 (Good) | Normal interval increase |
| Correct | Yes | 3 (Hard) | Slightly reduced interval |
| CloseEnough | No | 4 (Good) | No penalty (you knew it) |
| CloseEnough | Yes | 3 (Hard) | Slightly reduced interval |
| PartialMatch | Any | 2 (Hard) | Shorter interval (knowledge gap) |
| Incorrect | Any | 0 (Again) | Reset to learning phase |

### Code

```rust
pub fn to_quality(&self, used_hint: bool) -> u8 {
    match (self, used_hint) {
        (Correct, false) => 4,       // Good
        (Correct, true) => 3,        // Hard
        (CloseEnough, false) => 4,   // Good (typo, no penalty)
        (CloseEnough, true) => 3,    // Hard
        (PartialMatch, _) => 2,      // Hard (knowledge gap)
        (Incorrect, _) => 0,         // Again
    }
}
```

---

## Hint System

Progressive hints reveal the answer gradually.

### Levels

| Level | Example for "to be [is, am, are]" |
|-------|-----------------------------------|
| 1 | `t____ (5 letters)` |
| 2 | Description or `to___` |
| 3 | `to be [is, am, are]` (full) |

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

## Visual Display

When displaying answers, grammar elements show accessible markers:

| Grammar | Display | Symbol |
|---------|---------|--------|
| `[a, b]` | `≈[a, b]` | Approximately (variants) |
| `word(s)` | `word≈(s)` | Approximately (suffix) |
| `(info)` | `ℹ(info)` | Information |
| `<ctx>` | `△<ctx>` | Triangle (disambiguation) |

### CSS Classes

```css
.variant-marker { color: var(--color-indigo); }
.variant-marker::before { content: "≈"; }
.disambig-marker { color: var(--color-amber); }
.disambig-marker::before { content: "△"; }
.info-marker { color: var(--color-muted); }
.info-marker::before { content: "ℹ"; }
```

---

## Multiple Choice Mode

When `main_answer` contains Korean text, the card automatically uses multiple-choice input instead of text input (since Korean requires IME).

### Strict Matching

Multiple choice uses exact match (no typo tolerance):

```rust
if input_method.is_strict() {
    form.answer == card.main_answer
} else {
    validate_answer(&form.answer, &card.main_answer)
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
validate_answer("g/k", "g / k") → Correct    // slash variants
validate_answer("g  /  k", "g / k") → Correct // normalized
```

### Case Insensitivity

```rust
validate_answer("G", "g / k") → Correct
validate_answer("SCHOOL", "school") → Correct
```

---

## Source Files

| File | Purpose |
|------|---------|
| `src/validation.rs` | Core validation logic, grammar parser, normalization |
| `src/filters.rs` | `format_answer_display()` for visual markers |
| `crates/offline-srs/src/lib.rs` | WASM validation for offline mode |
| `doc/12_content_authoring.md` | Pack authoring guide |
