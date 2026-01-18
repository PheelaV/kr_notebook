"""Convert vocabulary.json to cards.json format.

This module converts vocabulary data into the flashcard format
expected by kr_notebook card packs.

Expected vocabulary.json format:
[
  {
    "term": "한국",
    "romanization": "hanguk",
    "translation": "Korea",
    "word_type": "Noun"
  },
  ...
]
"""

import json
from pathlib import Path
from typing import Any


def load_vocabulary(vocab_path: Path) -> list[dict[str, Any]]:
    """Load vocabulary entries from JSON file."""
    with open(vocab_path, encoding="utf-8") as f:
        return json.load(f)


def load_vocabulary_directory(dir_path: Path) -> list[dict[str, Any]]:
    """Load and merge all lesson_*.json files from a directory.

    Files are processed in sorted order (lesson_01.json, lesson_02.json, etc.).
    If an entry doesn't have a 'lesson' field, it's auto-populated from the filename.

    Args:
        dir_path: Path to directory containing lesson_*.json files

    Returns:
        Merged list of all vocabulary entries
    """
    all_vocab: list[dict[str, Any]] = []

    lesson_files = sorted(dir_path.glob("lesson_*.json"))
    if not lesson_files:
        raise ValueError(f"No lesson_*.json files found in {dir_path}")

    for lesson_file in lesson_files:
        vocab = load_vocabulary(lesson_file)
        # Extract lesson number from filename (e.g., lesson_01.json -> 1)
        lesson_num = int(lesson_file.stem.split("_")[1])
        for entry in vocab:
            if "lesson" not in entry:
                entry["lesson"] = lesson_num
        all_vocab.extend(vocab)

    return all_vocab


def create_card(
    vocab: dict[str, Any],
    tier: int,
    is_reverse: bool,
) -> dict[str, Any]:
    """Create a single card from vocabulary entry.

    Args:
        vocab: Vocabulary entry with term, romanization, translation, word_type
        tier: Card tier (default 5 for vocabulary)
        is_reverse: If True, creates English -> Korean card

    Returns:
        Card definition dict
    """
    if is_reverse:
        # English -> Korean direction
        card = {
            "front": vocab["translation"],
            "main_answer": vocab["term"],
            "description": f"({vocab['romanization']}) - {vocab['word_type']}",
            "card_type": "Vocabulary",
            "tier": tier,
            "is_reverse": True,
            "audio_hint": None,
        }
    else:
        # Korean -> English direction
        card = {
            "front": vocab["term"],
            "main_answer": vocab["translation"],
            "description": f"({vocab['romanization']}) - {vocab['word_type']}",
            "card_type": "Vocabulary",
            "tier": tier,
            "is_reverse": False,
            "audio_hint": None,
        }
    # Add lesson if present in vocabulary entry
    if "lesson" in vocab:
        card["lesson"] = vocab["lesson"]
    return card


def convert_vocabulary(
    vocab_path: Path,
    output_path: Path,
    tier: int = 5,
    create_reverse: bool = True,
) -> dict[str, Any]:
    """Convert vocabulary JSON to cards.json format.

    Args:
        vocab_path: Path to vocabulary.json file OR directory containing lesson_*.json files
        output_path: Path to write cards.json output
        tier: Card tier (default 5)
        create_reverse: Whether to create reverse cards (default True)

    Returns:
        Dict with conversion stats: vocabulary_count, cards_created, output
    """
    if vocab_path.is_dir():
        vocabulary = load_vocabulary_directory(vocab_path)
    else:
        vocabulary = load_vocabulary(vocab_path)

    cards: list[dict[str, Any]] = []

    for vocab in vocabulary:
        # Forward card: Korean -> English
        cards.append(create_card(vocab, tier, is_reverse=False))

        # Reverse card: English -> Korean (if enabled)
        if create_reverse:
            cards.append(create_card(vocab, tier, is_reverse=True))

    result = {"cards": cards}

    # Ensure output directory exists
    output_path.parent.mkdir(parents=True, exist_ok=True)

    with open(output_path, "w", encoding="utf-8") as f:
        json.dump(result, f, ensure_ascii=False, indent=2)

    return {
        "vocabulary_count": len(vocabulary),
        "cards_created": len(cards),
        "output": str(output_path),
    }
