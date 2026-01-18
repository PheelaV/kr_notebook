# Content Authoring Guide

This guide explains how to author content for kr_notebook, including the answer grammar syntax for defining flexible, user-friendly flashcard answers.

## Answer Grammar Syntax

The `main_answer` field supports a structured grammar that lets you define acceptable answer variants, optional suffixes, and supplementary information.

### Grammar Elements

| Syntax | Type | Validation | Display |
|--------|------|------------|---------|
| `[a, b, c]` | **Variants** | Any item is correct | Shows `≈[a, b, c]` |
| `word(s)` | **Suffix** | With/without suffix | Shows `word≈(s)` |
| `(info)` | **Info** | Ignored | Shows `ℹ(info)` |
| `<context>` | **Disambiguation** | Required for full credit | Shows `△<context>` |
| `a, b` | **Synonyms** | Any order accepted | Shows `a, b` |

### Visual Indicators

When displayed in the UI, grammar elements show accessible visual markers:
- **≈** (approximately): Marks acceptable variants and optional suffixes
- **ℹ** (info): Marks supplementary information that's not tested
- **△** (triangle): Marks disambiguation context (tested for partial/full credit)

### Examples

#### Variants with Brackets

Use `[...]` when multiple forms are all acceptable:

```json
{
  "front": "이다",
  "main_answer": "to be [is, am, are, was, were]"
}
```

Valid answers: "to be", "is", "am", "are", "was", "were"

#### Optional Suffix

Use `word(s)` (no space before paren) for optional endings:

```json
{
  "front": "눈",
  "main_answer": "eye(s)"
}
```

Valid answers: "eye", "eyes"

#### Information in Parentheses

Use `(info)` (with space before paren) for non-tested supplementary info:

```json
{
  "front": "저/제",
  "main_answer": "I, me (formal)"
}
```

Valid answers: "I", "me", "I me", "me I" (any order)
Invalid answers: "formal" (info is not tested)

#### Comma-Separated Synonyms

Synonyms separated by commas accept any order:

```json
{
  "front": "소파",
  "main_answer": "sofa, couch"
}
```

Valid answers: "sofa", "couch", "sofa couch", "couch sofa"

#### Disambiguation Context

Use `<context>` when users need additional context for full credit:

```json
{
  "front": "저",
  "main_answer": "that <far>"
}
```

- Full credit: "that far", "far that"
- Partial credit (PartialMatch): "that" (core correct but missing context)

### Parsing Rules

1. **Brackets `[...]`**: Content is split by comma, each item is an acceptable variant
2. **Suffix `word(...)`**: No space before opening paren means optional suffix
3. **Info `word (...)`**: Space before opening paren means supplementary info (ignored in validation)
4. **Angle brackets `<...>`**: Disambiguation context required for full credit
5. **Commas**: Top-level commas create synonyms that accept any order

### Normalization

The validation system automatically normalizes:

1. **Case insensitivity**: "HELLO" matches "hello"
2. **Whitespace**: Extra spaces are trimmed and collapsed
3. **British/American spellings**: "colour" matches "color"
4. **Contractions**: "I'm" matches "I am"

### Typo Tolerance

Answers are validated with Levenshtein distance tolerance:
- 1-character answers: Exact match required
- 2-4 character answers: 1 typo allowed
- 5+ character answers: 2 typos allowed

### Quality Rating Mapping

| Result | Quality | SRS Impact |
|--------|---------|------------|
| Correct | 4 | Normal interval increase |
| Correct (with hint) | 3 | Slightly reduced interval |
| CloseEnough (typo) | 4 | No penalty (you knew it) |
| PartialMatch | 2 | Shorter interval (knowledge gap) |
| Incorrect | 0 | Reset to learning steps |

## Card Structure

### Required Fields

```json
{
  "front": "학교",
  "main_answer": "school",
  "card_type": "Vocabulary",
  "tier": 5
}
```

### Optional Fields

```json
{
  "front": "학교",
  "main_answer": "school",
  "description": "(hak-gyo) - noun",
  "card_type": "Vocabulary",
  "tier": 5,
  "is_reverse": false,
  "audio_hint": "school.mp3",
  "lesson": 1
}
```

- **description**: Romanization, part of speech, or additional context
- **is_reverse**: `true` for English-to-Korean cards
- **audio_hint**: Audio file for pronunciation
- **lesson**: Lesson number for progressive unlocking

## Best Practices

### DO:

1. **Use variants for conjugations**: `to be [is, am, are, was, were]`
2. **Use synonyms for true alternatives**: `sofa, couch`
3. **Use info for romanization**: `학교 (hak-gyo)`
4. **Use info for grammar tags**: `run (verb)`
5. **Use suffix for plurals**: `book(s)`
6. **Keep main_answer focused**: Don't overload with too many variants

### DON'T:

1. **Put romanization in main_answer**: Move to description field
2. **Mix disambiguation with info**: `that (far)` vs `that <far>` mean different things
3. **Nest brackets**: `[[a, b], c]` - not supported
4. **Use brackets in description**: Description is free-form text

## Special Cases

### Phonetic Modifiers

Korean consonant cards with phonetic modifiers (tense/aspirated) are handled specially:

```json
{
  "front": "ㄲ",
  "main_answer": "kk (tense)"
}
```

Both the letter and modifier must be correct.

### Korean Answers

When `main_answer` is Korean, the card automatically uses multiple-choice input instead of text input (since Korean input requires IME).

### Reverse Cards

For vocabulary learning, create both directions:

```json
[
  {"front": "학교", "main_answer": "school", "is_reverse": false},
  {"front": "school", "main_answer": "학교", "is_reverse": true}
]
```

## Testing Your Content

1. Run `cargo test` to verify JSON syntax
2. Start the server and navigate to Settings > Content Packs
3. Enable your pack and test in Study mode
4. Check that variants, suffixes, and synonyms are accepted correctly
