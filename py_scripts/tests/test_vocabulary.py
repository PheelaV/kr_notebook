"""Tests for vocabulary card generation.

These tests ensure that the vocabulary.py module correctly handles lesson numbers
when generating cards from vocabulary.json files. This prevents regression of the
bug where lesson fields were not included in generated cards.
"""

import json
import tempfile
from pathlib import Path

from kr_scraper.vocabulary import (
    convert_vocabulary,
    create_card,
    load_vocabulary_directory,
)


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


class TestLoadVocabularyDirectory:
    """Tests for load_vocabulary_directory function."""

    def test_loads_multiple_lesson_files(self):
        """Load and merge vocabulary from multiple lesson files."""
        with tempfile.TemporaryDirectory() as tmpdir:
            vocab_dir = Path(tmpdir) / "vocabulary"
            vocab_dir.mkdir()

            # Create lesson_01.json
            lesson1 = [
                {"term": "A", "romanization": "a", "translation": "a", "word_type": "Noun", "lesson": 1},
            ]
            (vocab_dir / "lesson_01.json").write_text(json.dumps(lesson1))

            # Create lesson_02.json
            lesson2 = [
                {"term": "B", "romanization": "b", "translation": "b", "word_type": "Noun", "lesson": 2},
            ]
            (vocab_dir / "lesson_02.json").write_text(json.dumps(lesson2))

            result = load_vocabulary_directory(vocab_dir)

            assert len(result) == 2
            terms = {v["term"] for v in result}
            assert terms == {"A", "B"}

    def test_auto_populates_lesson_from_filename(self):
        """When lesson field is missing, infer from filename."""
        with tempfile.TemporaryDirectory() as tmpdir:
            vocab_dir = Path(tmpdir) / "vocabulary"
            vocab_dir.mkdir()

            # Create lesson without lesson field
            lesson3 = [
                {"term": "C", "romanization": "c", "translation": "c", "word_type": "Noun"},
            ]
            (vocab_dir / "lesson_03.json").write_text(json.dumps(lesson3))

            result = load_vocabulary_directory(vocab_dir)

            assert len(result) == 1
            assert result[0]["lesson"] == 3

    def test_preserves_existing_lesson_field(self):
        """When lesson field exists, don't override it."""
        with tempfile.TemporaryDirectory() as tmpdir:
            vocab_dir = Path(tmpdir) / "vocabulary"
            vocab_dir.mkdir()

            # File is lesson_05 but entry says lesson 99
            lesson5 = [
                {"term": "D", "romanization": "d", "translation": "d", "word_type": "Noun", "lesson": 99},
            ]
            (vocab_dir / "lesson_05.json").write_text(json.dumps(lesson5))

            result = load_vocabulary_directory(vocab_dir)

            assert result[0]["lesson"] == 99  # Preserve existing value

    def test_sorted_file_order(self):
        """Files should be processed in sorted order."""
        with tempfile.TemporaryDirectory() as tmpdir:
            vocab_dir = Path(tmpdir) / "vocabulary"
            vocab_dir.mkdir()

            # Create files out of order
            (vocab_dir / "lesson_03.json").write_text(json.dumps([
                {"term": "C", "romanization": "c", "translation": "c", "word_type": "Noun"}
            ]))
            (vocab_dir / "lesson_01.json").write_text(json.dumps([
                {"term": "A", "romanization": "a", "translation": "a", "word_type": "Noun"}
            ]))
            (vocab_dir / "lesson_02.json").write_text(json.dumps([
                {"term": "B", "romanization": "b", "translation": "b", "word_type": "Noun"}
            ]))

            result = load_vocabulary_directory(vocab_dir)

            terms = [v["term"] for v in result]
            assert terms == ["A", "B", "C"]  # Sorted order

    def test_raises_error_for_empty_directory(self):
        """Raise error when no lesson files found."""
        import pytest

        with tempfile.TemporaryDirectory() as tmpdir:
            vocab_dir = Path(tmpdir) / "vocabulary"
            vocab_dir.mkdir()

            with pytest.raises(ValueError, match="No lesson_\\*\\.json files found"):
                load_vocabulary_directory(vocab_dir)


class TestConvertVocabularyDirectory:
    """Tests for convert_vocabulary with directory input."""

    def test_converts_directory_to_cards(self):
        """Full pipeline: directory of lesson files → cards.json."""
        with tempfile.TemporaryDirectory() as tmpdir:
            vocab_dir = Path(tmpdir) / "vocabulary"
            vocab_dir.mkdir()
            output_path = Path(tmpdir) / "cards.json"

            # Create two lesson files
            (vocab_dir / "lesson_01.json").write_text(json.dumps([
                {"term": "A", "romanization": "a", "translation": "a", "word_type": "Noun"}
            ]))
            (vocab_dir / "lesson_02.json").write_text(json.dumps([
                {"term": "B", "romanization": "b", "translation": "b", "word_type": "Noun"}
            ]))

            result = convert_vocabulary(vocab_dir, output_path, create_reverse=False)

            assert result["vocabulary_count"] == 2
            assert result["cards_created"] == 2

            cards = json.loads(output_path.read_text())["cards"]
            assert len(cards) == 2

            # Verify lessons were auto-populated
            lessons = {c["lesson"] for c in cards}
            assert lessons == {1, 2}
