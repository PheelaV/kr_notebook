"""Tests for the segment module."""

import pytest

from kr_scraper.segment import (
    SegmentTimestamp,
    SegmentResult,
    romanize_syllable,
)
from pathlib import Path


class TestSegmentTimestamp:
    """Tests for SegmentTimestamp dataclass."""

    def test_create_timestamp(self):
        """Test creating a SegmentTimestamp."""
        ts = SegmentTimestamp(
            start_ms=100,
            end_ms=500,
            padded_start_ms=50,
            padded_end_ms=550,
        )
        assert ts.start_ms == 100
        assert ts.end_ms == 500
        assert ts.padded_start_ms == 50
        assert ts.padded_end_ms == 550


class TestSegmentResult:
    """Tests for SegmentResult dataclass."""

    def test_create_result_with_timestamps(self):
        """Test creating a SegmentResult with timestamps."""
        ts = SegmentTimestamp(
            start_ms=100,
            end_ms=500,
            padded_start_ms=50,
            padded_end_ms=550,
        )
        result = SegmentResult(
            source_file=Path("test.mp3"),
            syllables=["가"],
            segments_found=1,
            segments_saved=1,
            output_files=[Path("ga.mp3")],
            timestamps={"ga": ts},
        )
        assert result.timestamps["ga"].start_ms == 100
        assert result.segments_saved == 1


class TestRomanizeSyllable:
    """Tests for romanize_syllable function."""

    def test_romanize_simple_vowel(self):
        """Test romanizing simple vowels."""
        assert romanize_syllable("ㅏ") == "a"
        assert romanize_syllable("ㅓ") == "eo"
        assert romanize_syllable("ㅗ") == "o"
        assert romanize_syllable("ㅜ") == "u"
        assert romanize_syllable("ㅡ") == "eu"
        assert romanize_syllable("ㅣ") == "i"

    def test_romanize_simple_consonant(self):
        """Test romanizing simple consonants."""
        assert romanize_syllable("ㄱ") == "g"
        assert romanize_syllable("ㄴ") == "n"
        assert romanize_syllable("ㄷ") == "d"
        assert romanize_syllable("ㄹ") == "r"
        assert romanize_syllable("ㅁ") == "m"
        assert romanize_syllable("ㅂ") == "b"
        assert romanize_syllable("ㅅ") == "s"
        assert romanize_syllable("ㅈ") == "j"
        assert romanize_syllable("ㅎ") == "h"

    def test_romanize_composed_syllable(self):
        """Test romanizing composed syllables."""
        assert romanize_syllable("가") == "ga"
        assert romanize_syllable("나") == "na"
        assert romanize_syllable("다") == "da"
        assert romanize_syllable("라") == "ra"
        assert romanize_syllable("마") == "ma"
        assert romanize_syllable("바") == "ba"
        assert romanize_syllable("사") == "sa"
        assert romanize_syllable("자") == "ja"
        assert romanize_syllable("하") == "ha"

    def test_romanize_complex_vowels(self):
        """Test romanizing syllables with complex vowels."""
        assert romanize_syllable("애") == "ae"
        assert romanize_syllable("에") == "e"
        assert romanize_syllable("와") == "wa"
        assert romanize_syllable("워") == "wo"
        assert romanize_syllable("위") == "wi"
        assert romanize_syllable("의") == "ui"

    def test_romanize_with_initial_ieung(self):
        """Test romanizing syllables with silent initial ㅇ."""
        # ㅇ as initial consonant is silent
        assert romanize_syllable("아") == "a"
        assert romanize_syllable("어") == "eo"
        assert romanize_syllable("오") == "o"
        assert romanize_syllable("우") == "u"

    def test_romanize_lesson3_vowels(self):
        """Test romanizing lesson 3 complex vowels."""
        # ㅐ with silent ㅇ initial
        assert romanize_syllable("애") == "ae"
        assert romanize_syllable("에") == "e"
        assert romanize_syllable("의") == "ui"
        assert romanize_syllable("외") == "oe"

    def test_romanize_with_consonants(self):
        """Test romanizing syllables with different consonants."""
        # 가 (g+a), 개 (g+ae)
        assert romanize_syllable("가") == "ga"
        assert romanize_syllable("개") == "gae"
        assert romanize_syllable("게") == "ge"

        # 나 (n+a), 내 (n+ae)
        assert romanize_syllable("나") == "na"
        assert romanize_syllable("내") == "nae"

        # 바 (b+a), 배 (b+ae)
        assert romanize_syllable("바") == "ba"
        assert romanize_syllable("배") == "bae"
