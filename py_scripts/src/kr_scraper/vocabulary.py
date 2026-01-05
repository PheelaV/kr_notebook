"""Convert HTSK vocabulary.json to cards.json format.

This module converts vocabulary data extracted from the HTSK PDF
into the flashcard format expected by the kr_notebook app.
"""

import json
from pathlib import Path
from typing import Any


def load_vocabulary(vocab_path: Path) -> list[dict[str, Any]]:
    """Load vocabulary entries from JSON file."""
    with open(vocab_path, encoding="utf-8") as f:
        return json.load(f)


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
        return {
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
        return {
            "front": vocab["term"],
            "main_answer": vocab["translation"],
            "description": f"({vocab['romanization']}) - {vocab['word_type']}",
            "card_type": "Vocabulary",
            "tier": tier,
            "is_reverse": False,
            "audio_hint": None,
        }


def convert_vocabulary(
    vocab_path: Path,
    output_path: Path,
    tier: int = 5,
    create_reverse: bool = True,
) -> dict[str, Any]:
    """Convert vocabulary.json to cards.json format.

    Args:
        vocab_path: Path to vocabulary.json source file
        output_path: Path to write cards.json output
        tier: Card tier (default 5)
        create_reverse: Whether to create reverse cards (default True)

    Returns:
        Dict with conversion stats: vocabulary_count, cards_created, output
    """
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
