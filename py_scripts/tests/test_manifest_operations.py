"""Tests for manifest manipulation operations.

These tests verify that segment operations correctly preserve existing data
and don't corrupt the manifest when updating individual syllables or rows.
"""

import json
import pytest
from pathlib import Path
from tempfile import TemporaryDirectory

from kr_scraper.segment import (
    SegmentTimestamp,
    update_manifest_with_segments,
    apply_manual_segment,
    reset_manual_segment,
)


class ManifestFixture:
    """Helper to create and manage test manifests."""

    def __init__(self, temp_dir: Path):
        self.lesson_dir = temp_dir
        self.manifest_path = temp_dir / "manifest.json"
        self.syllables_dir = temp_dir / "syllables"
        self.rows_dir = temp_dir / "rows"
        self.syllables_dir.mkdir()
        self.rows_dir.mkdir()

    def create_manifest(self, manifest: dict) -> None:
        """Write manifest to disk."""
        with open(self.manifest_path, "w", encoding="utf-8") as f:
            json.dump(manifest, f, ensure_ascii=False, indent=2)

    def load_manifest(self) -> dict:
        """Read manifest from disk."""
        with open(self.manifest_path, encoding="utf-8") as f:
            return json.load(f)

    def create_dummy_audio(self, filename: str, duration_ms: int = 1000) -> Path:
        """Create a dummy audio file (just empty bytes for testing)."""
        # For actual audio operations we'd need pydub, but for manifest tests
        # we just need the file to exist
        path = self.rows_dir / filename
        path.write_bytes(b"\x00" * 100)  # Dummy content
        return path

    def create_syllable_audio(self, romanization: str) -> Path:
        """Create a dummy syllable audio file."""
        path = self.syllables_dir / f"{romanization}.mp3"
        path.write_bytes(b"\x00" * 100)
        return path


@pytest.fixture
def temp_lesson_dir():
    """Create a temporary directory for test files."""
    with TemporaryDirectory() as tmp:
        yield Path(tmp)


@pytest.fixture
def manifest_fixture(temp_lesson_dir):
    """Create a ManifestFixture instance."""
    return ManifestFixture(temp_lesson_dir)


def create_base_manifest() -> dict:
    """Create a base manifest structure for testing."""
    return {
        "lesson": "test_lesson",
        "scraped_at": "2024-01-01T00:00:00",
        "vowels_order": ["ㅏ", "ㅓ"],
        "consonants_order": [],
        "rows": {
            "ㅏ": {
                "file": "rows/row_a.mp3",
                "romanization": "a",
                "syllables": ["가", "나", "다"],
                "segment_params": {"min_silence": 200, "threshold": -40, "padding": 75},
            },
            "ㅓ": {
                "file": "rows/row_eo.mp3",
                "romanization": "eo",
                "syllables": ["거", "너", "더"],
                "segment_params": {"min_silence": 200, "threshold": -40, "padding": 75},
            },
        },
        "columns": {},
        "syllable_table": {
            "가": {
                "consonant": "ㄱ",
                "vowel": "ㅏ",
                "romanization": "ga",
                "segment": {
                    "file": "syllables/ga.mp3",
                    "baseline": {
                        "start_ms": 0,
                        "end_ms": 400,
                        "padded_start_ms": 0,
                        "padded_end_ms": 475,
                    },
                },
            },
            "나": {
                "consonant": "ㄴ",
                "vowel": "ㅏ",
                "romanization": "na",
                "segment": {
                    "file": "syllables/na.mp3",
                    "baseline": {
                        "start_ms": 500,
                        "end_ms": 900,
                        "padded_start_ms": 425,
                        "padded_end_ms": 975,
                    },
                },
            },
            "다": {
                "consonant": "ㄷ",
                "vowel": "ㅏ",
                "romanization": "da",
                "segment": {
                    "file": "syllables/da.mp3",
                    "baseline": {
                        "start_ms": 1000,
                        "end_ms": 1400,
                        "padded_start_ms": 925,
                        "padded_end_ms": 1475,
                    },
                },
            },
            "거": {
                "consonant": "ㄱ",
                "vowel": "ㅓ",
                "romanization": "geo",
                "segment": {
                    "file": "syllables/geo.mp3",
                    "baseline": {
                        "start_ms": 0,
                        "end_ms": 450,
                        "padded_start_ms": 0,
                        "padded_end_ms": 525,
                    },
                },
            },
            "너": {
                "consonant": "ㄴ",
                "vowel": "ㅓ",
                "romanization": "neo",
                "segment": {
                    "file": "syllables/neo.mp3",
                    "baseline": {
                        "start_ms": 550,
                        "end_ms": 950,
                        "padded_start_ms": 475,
                        "padded_end_ms": 1025,
                    },
                },
            },
            "더": {
                "consonant": "ㄷ",
                "vowel": "ㅓ",
                "romanization": "deo",
                "segment": {
                    "file": "syllables/deo.mp3",
                    "baseline": {
                        "start_ms": 1050,
                        "end_ms": 1450,
                        "padded_start_ms": 975,
                        "padded_end_ms": 1525,
                    },
                },
            },
        },
    }


class TestUpdateManifestWithSegments:
    """Tests for update_manifest_with_segments function."""

    def test_updates_only_specified_syllables(self, manifest_fixture):
        """Syllables not in all_timestamps should be left unchanged."""
        manifest = create_base_manifest()
        manifest_fixture.create_manifest(manifest)

        # Create audio files for the syllables we're updating
        manifest_fixture.create_syllable_audio("ga")

        # Only update 'ga', leave others alone
        new_timestamps = {
            "ga": SegmentTimestamp(
                start_ms=10,
                end_ms=410,
                padded_start_ms=0,
                padded_end_ms=485,
            ),
        }

        update_manifest_with_segments(
            manifest_fixture.manifest_path,
            manifest_fixture.syllables_dir,
            new_timestamps,
        )

        result = manifest_fixture.load_manifest()

        # 'ga' should be updated with new baseline
        assert result["syllable_table"]["가"]["segment"]["baseline"]["start_ms"] == 10
        assert result["syllable_table"]["가"]["segment"]["baseline"]["end_ms"] == 410

        # 'na' should retain original baseline (not in all_timestamps)
        assert result["syllable_table"]["나"]["segment"]["baseline"]["start_ms"] == 500
        assert result["syllable_table"]["나"]["segment"]["baseline"]["end_ms"] == 900

        # 'geo' from other row should also be unchanged
        assert result["syllable_table"]["거"]["segment"]["baseline"]["start_ms"] == 0
        assert result["syllable_table"]["거"]["segment"]["baseline"]["end_ms"] == 450

    def test_preserves_manual_overrides_when_updating_baseline(self, manifest_fixture):
        """Manual overrides should be preserved when baseline is updated."""
        manifest = create_base_manifest()
        # Add a manual override to 'ga'
        manifest["syllable_table"]["가"]["segment"]["manual"] = {
            "start_ms": 20,
            "end_ms": 380,
            "padded_start_ms": 0,
            "padded_end_ms": 455,
        }
        manifest_fixture.create_manifest(manifest)
        manifest_fixture.create_syllable_audio("ga")

        # Update baseline for 'ga'
        new_timestamps = {
            "ga": SegmentTimestamp(
                start_ms=15,
                end_ms=405,
                padded_start_ms=0,
                padded_end_ms=480,
            ),
        }

        update_manifest_with_segments(
            manifest_fixture.manifest_path,
            manifest_fixture.syllables_dir,
            new_timestamps,
        )

        result = manifest_fixture.load_manifest()

        # Baseline should be updated
        assert result["syllable_table"]["가"]["segment"]["baseline"]["start_ms"] == 15

        # Manual override should be preserved
        assert "manual" in result["syllable_table"]["가"]["segment"]
        assert result["syllable_table"]["가"]["segment"]["manual"]["start_ms"] == 20

    def test_empty_timestamps_leaves_all_unchanged(self, manifest_fixture):
        """Empty all_timestamps dict should not modify any syllables."""
        manifest = create_base_manifest()
        original_ga_baseline = manifest["syllable_table"]["가"]["segment"]["baseline"].copy()
        original_neo_baseline = manifest["syllable_table"]["너"]["segment"]["baseline"].copy()
        manifest_fixture.create_manifest(manifest)

        update_manifest_with_segments(
            manifest_fixture.manifest_path,
            manifest_fixture.syllables_dir,
            {},  # Empty timestamps
        )

        result = manifest_fixture.load_manifest()

        # All syllables should retain original baselines
        assert result["syllable_table"]["가"]["segment"]["baseline"] == original_ga_baseline
        assert result["syllable_table"]["너"]["segment"]["baseline"] == original_neo_baseline

    def test_updates_multiple_syllables_from_same_row(self, manifest_fixture):
        """Should correctly update multiple syllables from the same row."""
        manifest = create_base_manifest()
        manifest_fixture.create_manifest(manifest)
        manifest_fixture.create_syllable_audio("ga")
        manifest_fixture.create_syllable_audio("na")
        manifest_fixture.create_syllable_audio("da")

        # Update all syllables from row 'a'
        new_timestamps = {
            "ga": SegmentTimestamp(start_ms=10, end_ms=400, padded_start_ms=0, padded_end_ms=475),
            "na": SegmentTimestamp(start_ms=510, end_ms=900, padded_start_ms=435, padded_end_ms=975),
            "da": SegmentTimestamp(start_ms=1010, end_ms=1400, padded_start_ms=935, padded_end_ms=1475),
        }

        update_manifest_with_segments(
            manifest_fixture.manifest_path,
            manifest_fixture.syllables_dir,
            new_timestamps,
        )

        result = manifest_fixture.load_manifest()

        # All row 'a' syllables updated
        assert result["syllable_table"]["가"]["segment"]["baseline"]["start_ms"] == 10
        assert result["syllable_table"]["나"]["segment"]["baseline"]["start_ms"] == 510
        assert result["syllable_table"]["다"]["segment"]["baseline"]["start_ms"] == 1010

        # Row 'eo' syllables unchanged
        assert result["syllable_table"]["거"]["segment"]["baseline"]["start_ms"] == 0
        assert result["syllable_table"]["너"]["segment"]["baseline"]["start_ms"] == 550


@pytest.fixture
def mock_audio(mocker):
    """Mock AudioSegment to avoid needing real audio files."""
    mock_segment = mocker.MagicMock()
    mock_segment.__getitem__ = mocker.MagicMock(return_value=mock_segment)
    mock_segment.__len__ = mocker.MagicMock(return_value=2000)  # 2 seconds
    mock_segment.export = mocker.MagicMock()

    mock = mocker.patch("kr_scraper.segment.AudioSegment")
    mock.from_mp3 = mocker.MagicMock(return_value=mock_segment)
    return mock


class TestApplyManualSegment:
    """Tests for apply_manual_segment function."""

    def test_applies_manual_preserving_baseline(self, manifest_fixture, mock_audio):
        """Applying manual should preserve existing baseline."""
        manifest = create_base_manifest()
        manifest_fixture.create_manifest(manifest)
        manifest_fixture.create_dummy_audio("row_a.mp3")

        original_baseline = manifest["syllable_table"]["가"]["segment"]["baseline"].copy()

        success = apply_manual_segment(
            manifest_fixture.lesson_dir,
            syllable="가",
            start_ms=50,
            end_ms=350,
            padding_ms=75,
        )

        assert success
        result = manifest_fixture.load_manifest()

        # Baseline should be preserved
        assert result["syllable_table"]["가"]["segment"]["baseline"] == original_baseline

        # Manual should be added
        assert "manual" in result["syllable_table"]["가"]["segment"]
        assert result["syllable_table"]["가"]["segment"]["manual"]["start_ms"] == 50
        assert result["syllable_table"]["가"]["segment"]["manual"]["end_ms"] == 350

    def test_applies_manual_to_syllable_without_segment(self, manifest_fixture, mock_audio):
        """Applying manual to syllable with no segment should create proper structure."""
        manifest = create_base_manifest()
        # Remove segment info entirely
        manifest["syllable_table"]["가"]["segment"] = None
        manifest_fixture.create_manifest(manifest)
        manifest_fixture.create_dummy_audio("row_a.mp3")

        success = apply_manual_segment(
            manifest_fixture.lesson_dir,
            syllable="가",
            start_ms=50,
            end_ms=350,
            padding_ms=75,
        )

        assert success
        result = manifest_fixture.load_manifest()

        # Should have proper structure with file key
        segment = result["syllable_table"]["가"]["segment"]
        assert segment is not None
        assert "file" in segment
        assert segment["file"] == "syllables/ga.mp3"
        assert "manual" in segment
        assert segment["manual"]["start_ms"] == 50

    def test_applies_manual_to_syllable_with_empty_segment(self, manifest_fixture, mock_audio):
        """Applying manual to syllable with empty segment dict should create proper structure."""
        manifest = create_base_manifest()
        # Set segment to empty dict
        manifest["syllable_table"]["가"]["segment"] = {}
        manifest_fixture.create_manifest(manifest)
        manifest_fixture.create_dummy_audio("row_a.mp3")

        success = apply_manual_segment(
            manifest_fixture.lesson_dir,
            syllable="가",
            start_ms=50,
            end_ms=350,
            padding_ms=75,
        )

        assert success
        result = manifest_fixture.load_manifest()

        # Should have proper structure with file key
        segment = result["syllable_table"]["가"]["segment"]
        assert "file" in segment
        assert segment["file"] == "syllables/ga.mp3"
        assert "manual" in segment

    def test_does_not_affect_other_syllables(self, manifest_fixture, mock_audio):
        """Applying manual to one syllable should not affect others."""
        manifest = create_base_manifest()
        manifest_fixture.create_manifest(manifest)
        manifest_fixture.create_dummy_audio("row_a.mp3")

        original_na = manifest["syllable_table"]["나"]["segment"].copy()
        original_geo = manifest["syllable_table"]["거"]["segment"].copy()

        apply_manual_segment(
            manifest_fixture.lesson_dir,
            syllable="가",
            start_ms=50,
            end_ms=350,
            padding_ms=75,
        )

        result = manifest_fixture.load_manifest()

        # Other syllables unchanged
        assert result["syllable_table"]["나"]["segment"] == original_na
        assert result["syllable_table"]["거"]["segment"] == original_geo

    def test_returns_false_for_unknown_syllable(self, manifest_fixture):
        """Should return False for syllable not in manifest."""
        manifest = create_base_manifest()
        manifest_fixture.create_manifest(manifest)

        success = apply_manual_segment(
            manifest_fixture.lesson_dir,
            syllable="xyz",  # Not in manifest
            start_ms=50,
            end_ms=350,
        )

        assert not success


class TestResetManualSegment:
    """Tests for reset_manual_segment function."""

    def test_removes_manual_preserving_baseline(self, manifest_fixture, mock_audio):
        """Resetting should remove manual but preserve baseline."""
        manifest = create_base_manifest()
        manifest["syllable_table"]["가"]["segment"]["manual"] = {
            "start_ms": 50,
            "end_ms": 350,
            "padded_start_ms": 0,
            "padded_end_ms": 425,
        }
        manifest_fixture.create_manifest(manifest)
        manifest_fixture.create_dummy_audio("row_a.mp3")

        original_baseline = manifest["syllable_table"]["가"]["segment"]["baseline"].copy()

        success = reset_manual_segment(
            manifest_fixture.lesson_dir,
            syllable="가",
        )

        assert success
        result = manifest_fixture.load_manifest()

        # Baseline preserved
        assert result["syllable_table"]["가"]["segment"]["baseline"] == original_baseline

        # Manual removed
        assert "manual" not in result["syllable_table"]["가"]["segment"]

    def test_returns_false_without_baseline(self, manifest_fixture):
        """Should return False if no baseline to reset to."""
        manifest = create_base_manifest()
        # Remove baseline, keep only manual
        manifest["syllable_table"]["가"]["segment"] = {
            "file": "syllables/ga.mp3",
            "manual": {"start_ms": 50, "end_ms": 350, "padded_start_ms": 0, "padded_end_ms": 425},
        }
        manifest_fixture.create_manifest(manifest)

        success = reset_manual_segment(
            manifest_fixture.lesson_dir,
            syllable="가",
        )

        assert not success

    def test_does_not_affect_other_syllables(self, manifest_fixture, mock_audio):
        """Resetting one syllable should not affect others."""
        manifest = create_base_manifest()
        # Add manual to multiple syllables
        manifest["syllable_table"]["가"]["segment"]["manual"] = {
            "start_ms": 50,
            "end_ms": 350,
            "padded_start_ms": 0,
            "padded_end_ms": 425,
        }
        manifest["syllable_table"]["나"]["segment"]["manual"] = {
            "start_ms": 550,
            "end_ms": 850,
            "padded_start_ms": 475,
            "padded_end_ms": 925,
        }
        manifest_fixture.create_manifest(manifest)
        manifest_fixture.create_dummy_audio("row_a.mp3")

        reset_manual_segment(
            manifest_fixture.lesson_dir,
            syllable="가",
        )

        result = manifest_fixture.load_manifest()

        # 'ga' manual removed
        assert "manual" not in result["syllable_table"]["가"]["segment"]

        # 'na' manual still present
        assert "manual" in result["syllable_table"]["나"]["segment"]
        assert result["syllable_table"]["나"]["segment"]["manual"]["start_ms"] == 550


class TestManifestIntegrity:
    """Integration tests for manifest integrity across operations."""

    def test_full_workflow_preserves_integrity(self, manifest_fixture, mock_audio):
        """Test a full workflow: segment row, apply manual, reset manual."""
        manifest = create_base_manifest()
        manifest_fixture.create_manifest(manifest)
        manifest_fixture.create_dummy_audio("row_a.mp3")
        manifest_fixture.create_syllable_audio("ga")
        manifest_fixture.create_syllable_audio("na")
        manifest_fixture.create_syllable_audio("da")

        # Step 1: Update segments for row 'a' (simulating re-segmentation)
        new_timestamps = {
            "ga": SegmentTimestamp(start_ms=5, end_ms=395, padded_start_ms=0, padded_end_ms=470),
            "na": SegmentTimestamp(start_ms=505, end_ms=895, padded_start_ms=430, padded_end_ms=970),
            "da": SegmentTimestamp(start_ms=1005, end_ms=1395, padded_start_ms=930, padded_end_ms=1470),
        }
        update_manifest_with_segments(
            manifest_fixture.manifest_path,
            manifest_fixture.syllables_dir,
            new_timestamps,
        )

        # Verify row 'eo' unchanged
        result = manifest_fixture.load_manifest()
        assert result["syllable_table"]["거"]["segment"]["baseline"]["start_ms"] == 0

        # Step 2: Apply manual adjustment to 'ga'
        apply_manual_segment(
            manifest_fixture.lesson_dir,
            syllable="가",
            start_ms=10,
            end_ms=380,
        )

        result = manifest_fixture.load_manifest()
        assert result["syllable_table"]["가"]["segment"]["manual"]["start_ms"] == 10
        assert result["syllable_table"]["가"]["segment"]["baseline"]["start_ms"] == 5  # Baseline preserved

        # Step 3: Re-segment row 'a' again - manual should be preserved
        new_timestamps_2 = {
            "ga": SegmentTimestamp(start_ms=8, end_ms=398, padded_start_ms=0, padded_end_ms=473),
            "na": SegmentTimestamp(start_ms=508, end_ms=898, padded_start_ms=433, padded_end_ms=973),
            "da": SegmentTimestamp(start_ms=1008, end_ms=1398, padded_start_ms=933, padded_end_ms=1473),
        }
        update_manifest_with_segments(
            manifest_fixture.manifest_path,
            manifest_fixture.syllables_dir,
            new_timestamps_2,
        )

        result = manifest_fixture.load_manifest()
        assert result["syllable_table"]["가"]["segment"]["baseline"]["start_ms"] == 8  # New baseline
        assert result["syllable_table"]["가"]["segment"]["manual"]["start_ms"] == 10  # Manual preserved

        # Step 4: Reset 'ga' to baseline
        reset_manual_segment(manifest_fixture.lesson_dir, syllable="가")

        result = manifest_fixture.load_manifest()
        assert "manual" not in result["syllable_table"]["가"]["segment"]
        assert result["syllable_table"]["가"]["segment"]["baseline"]["start_ms"] == 8

        # Final check: row 'eo' still unchanged throughout
        assert result["syllable_table"]["거"]["segment"]["baseline"]["start_ms"] == 0
        assert result["syllable_table"]["너"]["segment"]["baseline"]["start_ms"] == 550
