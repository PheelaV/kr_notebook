"""Tests for vocabulary card generation.

These tests ensure that the vocabulary.py module correctly handles lesson numbers
when generating cards from vocabulary.json files. This prevents regression of the
bug where lesson fields were not included in generated cards.
"""

import json
import tempfile
from pathlib import Path

from kr_scraper.vocabulary import convert_vocabulary, create_card


class TestCreateCard:
    """Tests for create_card function."""

    def test_includes_lesson_when_present(self):
        """Verify create_card includes lesson field when present in vocab."""
        vocab = {
            "term": "한국",
            "romanization": "hanguk",
            "translation": "Korea",
            "word_type": "Noun",
            "lesson": 1,
        }
        card = create_card(vocab, tier=5, is_reverse=False)

        assert "lesson" in card, "Card must include lesson field when vocab has it"
        assert card["lesson"] == 1

    def test_includes_lesson_for_reverse_card(self):
        """Verify reverse cards also include lesson field."""
        vocab = {
            "term": "사람",
            "romanization": "saram",
            "translation": "person",
            "word_type": "Noun",
            "lesson": 3,
        }
        card = create_card(vocab, tier=5, is_reverse=True)

        assert "lesson" in card, "Reverse card must include lesson field"
        assert card["lesson"] == 3

    def test_no_lesson_when_not_present(self):
        """Verify create_card works without lesson field in vocab."""
        vocab = {
            "term": "한국",
            "romanization": "hanguk",
            "translation": "Korea",
            "word_type": "Noun",
        }
        card = create_card(vocab, tier=5, is_reverse=False)

        # Should not have lesson key, or it should be None
        assert "lesson" not in card or card.get("lesson") is None

    def test_preserves_lesson_number_value(self):
        """Verify various lesson numbers are preserved correctly."""
        for lesson_num in [1, 2, 5, 8, 10]:
            vocab = {
                "term": "test",
                "romanization": "test",
                "translation": "test",
                "word_type": "Noun",
                "lesson": lesson_num,
            }
            card = create_card(vocab, tier=5, is_reverse=False)
            assert card["lesson"] == lesson_num, f"Lesson {lesson_num} not preserved"


class TestConvertVocabulary:
    """Tests for convert_vocabulary function (full pipeline)."""

    def test_preserves_lessons_in_output(self):
        """Full pipeline test: vocabulary.json → cards.json with lessons."""
        vocab_data = [
            {
                "term": "A",
                "romanization": "a",
                "translation": "a",
                "word_type": "Noun",
                "lesson": 1,
            },
            {
                "term": "B",
                "romanization": "b",
                "translation": "b",
                "word_type": "Noun",
                "lesson": 2,
            },
        ]

        with tempfile.TemporaryDirectory() as tmpdir:
            vocab_path = Path(tmpdir) / "vocabulary.json"
            output_path = Path(tmpdir) / "cards.json"

            vocab_path.write_text(json.dumps(vocab_data))
            convert_vocabulary(vocab_path, output_path, create_reverse=False)

            cards = json.loads(output_path.read_text())["cards"]

            lessons = {c.get("lesson") for c in cards}
            assert lessons == {1, 2}, f"Expected lessons {{1, 2}}, got {lessons}"

    def test_reverse_cards_have_lessons(self):
        """Verify reverse cards also get lesson numbers."""
        vocab_data = [
            {
                "term": "한국",
                "romanization": "hanguk",
                "translation": "Korea",
                "word_type": "Noun",
                "lesson": 1,
            },
        ]

        with tempfile.TemporaryDirectory() as tmpdir:
            vocab_path = Path(tmpdir) / "vocabulary.json"
            output_path = Path(tmpdir) / "cards.json"

            vocab_path.write_text(json.dumps(vocab_data))
            convert_vocabulary(vocab_path, output_path, create_reverse=True)

            cards = json.loads(output_path.read_text())["cards"]

            # Should have 2 cards (forward + reverse), both with lesson 1
            assert len(cards) == 2
            for card in cards:
                assert card.get("lesson") == 1, "Both cards should have lesson 1"

    def test_mixed_lessons_and_no_lessons(self):
        """Vocabulary with some items having lessons and some not."""
        vocab_data = [
            {
                "term": "A",
                "romanization": "a",
                "translation": "a",
                "word_type": "Noun",
                "lesson": 1,
            },
            {
                "term": "B",
                "romanization": "b",
                "translation": "b",
                "word_type": "Noun",
                # No lesson field
            },
        ]

        with tempfile.TemporaryDirectory() as tmpdir:
            vocab_path = Path(tmpdir) / "vocabulary.json"
            output_path = Path(tmpdir) / "cards.json"

            vocab_path.write_text(json.dumps(vocab_data))
            convert_vocabulary(vocab_path, output_path, create_reverse=False)

            cards = json.loads(output_path.read_text())["cards"]

            # First card should have lesson, second should not
            card_a = next(c for c in cards if c["front"] == "A")
            card_b = next(c for c in cards if c["front"] == "B")

            assert card_a.get("lesson") == 1
            assert "lesson" not in card_b or card_b.get("lesson") is None
