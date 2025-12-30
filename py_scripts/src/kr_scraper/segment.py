"""Audio segmentation for extracting individual syllables from row/column audio."""

import json
from dataclasses import dataclass, field
from pathlib import Path
from typing import Callable

from pydub import AudioSegment
from pydub.silence import detect_nonsilent

from .lesson1 import ROMANIZATION


# Default audio file quirks that require special handling.
# These are fallback values - manifest segment_params take precedence.
# Format: "filename" -> {
#   "skip_first": N,      # Skip N segments from start
#   "skip_last": N,       # Skip N segments from end
#   "use_indices": [0,1,2...],  # Use only these segment indices
#   "min_silence": N,     # Override min_silence_len (ms)
#   "threshold": N,       # Override silence_thresh (dBFS)
# }
DEFAULT_AUDIO_OVERRIDES: dict[str, dict] = {
    # The 'd' row (ㄷ) has noise at the start creating an extra segment
    "row_d.mp3": {"skip_first": 1},
    # The 'k' row (ㅋ) speaker is fast - needs shorter silence detection
    "row_k.mp3": {"min_silence": 150},
}


def romanize_syllable(syllable: str) -> str:
    """Convert a Korean syllable to romanization.

    For single jamo (ㄱ, ㅏ, etc.), uses the standard mapping.
    For composed syllables (가, 비, etc.), decomposes and romanizes.

    Args:
        syllable: Korean character to romanize.

    Returns:
        Romanized string safe for filenames.
    """
    # If it's a single jamo, use direct mapping
    if syllable in ROMANIZATION:
        return ROMANIZATION[syllable]

    # Try to decompose composed syllables
    if len(syllable) == 1:
        code = ord(syllable)
        # Check if it's a composed Hangul syllable (AC00-D7A3)
        if 0xAC00 <= code <= 0xD7A3:
            # Decompose into choseong (initial), jungseong (medial), jongseong (final)
            syllable_index = code - 0xAC00
            cho_index = syllable_index // 588
            jung_index = (syllable_index % 588) // 28
            jong_index = syllable_index % 28

            # Initial consonants (Revised Romanization of Korean)
            # Order: ㄱㄲㄴㄷㄸㄹㅁㅂㅃㅅㅆㅇㅈㅉㅊㅋㅌㅍㅎ
            CHOSEONG_ROM = [
                "g", "kk", "n", "d", "tt", "r", "m", "b", "pp", "s",
                "ss", "", "j", "jj", "ch", "k", "t", "p", "h",
            ]
            # Vowels
            JUNGSEONG_ROM = [
                "a", "ae", "ya", "yae", "eo", "e", "yeo", "ye", "o", "wa",
                "wae", "oe", "yo", "u", "wo", "we", "wi", "yu", "eu", "ui", "i",
            ]
            # Final consonants (first is empty = no final)
            JONGSEONG_ROM = [
                "", "g", "gg", "gs", "n", "nj", "nh", "d", "r", "rg",
                "rm", "rb", "rs", "rt", "rp", "rh", "m", "b", "bs", "s",
                "ss", "ng", "j", "ch", "k", "t", "p", "h",
            ]

            cho = CHOSEONG_ROM[cho_index]
            jung = JUNGSEONG_ROM[jung_index]
            jong = JONGSEONG_ROM[jong_index]

            return f"{cho}{jung}{jong}"

    # Fallback: use the syllable itself (may have encoding issues)
    return syllable


@dataclass
class SegmentTimestamp:
    """Timestamp info for a single segment."""

    start_ms: int
    end_ms: int
    padded_start_ms: int
    padded_end_ms: int


@dataclass
class SegmentResult:
    """Result of segmenting an audio file."""

    source_file: Path
    syllables: list[str]
    segments_found: int
    segments_saved: int
    output_files: list[Path]
    source_label: str = ""  # e.g., "row:ㄷ" or "col:ㅏ"
    mismatch: bool = False
    override_applied: str | None = None
    skipped_segments: int = 0
    # Effective parameters used (for storing back to manifest)
    effective_params: dict = field(default_factory=dict)
    # Timestamps for each saved segment (romanization -> SegmentTimestamp)
    timestamps: dict[str, SegmentTimestamp] = field(default_factory=dict)


def detect_syllable_boundaries(
    audio: AudioSegment,
    min_silence_len: int = 200,
    silence_thresh: int = -40,
    seek_step: int = 10,
) -> list[tuple[int, int]]:
    """Detect non-silent regions (syllables) in audio.

    Args:
        audio: The audio to analyze.
        min_silence_len: Minimum silence duration (ms) to split on.
        silence_thresh: dBFS threshold for silence detection.
        seek_step: Step size for silence detection (ms).

    Returns:
        List of (start_ms, end_ms) tuples for each non-silent region.
    """
    return detect_nonsilent(
        audio,
        min_silence_len=min_silence_len,
        silence_thresh=silence_thresh,
        seek_step=seek_step,
    )


def segment_audio_file(
    audio_path: Path,
    syllables: list[str],
    output_dir: Path,
    min_silence_len: int = 200,
    silence_thresh: int = -40,
    padding_ms: int = 50,
    source_label: str = "",
    manifest_params: dict | None = None,
) -> SegmentResult:
    """Segment an audio file into individual syllables.

    Args:
        audio_path: Path to the source audio file.
        syllables: List of syllable names in order of appearance.
        output_dir: Directory to save segmented files.
        min_silence_len: Minimum silence duration (ms) to split on.
        silence_thresh: dBFS threshold for silence detection.
        padding_ms: Padding to add before/after each segment.
        source_label: Label for error messages (e.g., "row:ㄷ").

    Returns:
        SegmentResult with details about the segmentation.
    """
    audio = AudioSegment.from_mp3(audio_path)

    # Check for overrides: manifest_params > DEFAULT_AUDIO_OVERRIDES > CLI args
    filename = audio_path.name
    default_override = DEFAULT_AUDIO_OVERRIDES.get(filename, {})

    # Merge: start with defaults, then manifest params override
    override = {**default_override}
    if manifest_params:
        override.update(manifest_params)

    override_applied = None
    skipped_segments = 0

    # Apply parameter overrides
    effective_min_silence = override.get("min_silence", min_silence_len)
    effective_threshold = override.get("threshold", silence_thresh)
    effective_padding = override.get("padding", padding_ms)

    if effective_min_silence != min_silence_len or effective_threshold != silence_thresh or effective_padding != padding_ms:
        params_override = []
        if effective_min_silence != min_silence_len:
            params_override.append(f"s={effective_min_silence}")
        if effective_threshold != silence_thresh:
            params_override.append(f"t={effective_threshold}")
        if effective_padding != padding_ms:
            params_override.append(f"P={effective_padding}")
        override_applied = ", ".join(params_override)

    # Detect syllable boundaries
    boundaries = detect_syllable_boundaries(
        audio,
        min_silence_len=effective_min_silence,
        silence_thresh=effective_threshold,
    )

    if override:
        original_count = len(boundaries)
        override_parts = []

        # Track parameter overrides if any
        if override_applied:
            override_parts.append(override_applied)

        # Apply skip_first
        if "skip_first" in override:
            skip = override["skip_first"]
            boundaries = boundaries[skip:]
            skipped_segments += skip
            override_parts.append(f"skip_first={skip}")

        # Apply skip_last
        if "skip_last" in override:
            skip = override["skip_last"]
            boundaries = boundaries[:-skip] if skip > 0 else boundaries
            skipped_segments += skip
            override_parts.append(f"skip_last={skip}")

        # Apply use_indices (explicit selection)
        if "use_indices" in override:
            indices = override["use_indices"]
            boundaries = [boundaries[i] for i in indices if i < len(boundaries)]
            override_parts.append(f"use_indices={indices}")

        if override_parts:
            override_applied = ", ".join(override_parts)

    output_dir.mkdir(parents=True, exist_ok=True)
    output_files = []
    segments_saved = 0
    timestamps: dict[str, SegmentTimestamp] = {}

    # Check for mismatch
    mismatch = len(boundaries) != len(syllables)

    # Match segments to syllables
    for i, (start_ms, end_ms) in enumerate(boundaries):
        if i >= len(syllables):
            # More segments than expected syllables
            break

        syllable = syllables[i]
        romanization = romanize_syllable(syllable)

        # Add padding (but don't go out of bounds)
        padded_start = max(0, start_ms - effective_padding)
        padded_end = min(len(audio), end_ms + effective_padding)

        segment = audio[padded_start:padded_end]

        # Create filename from syllable romanization
        filename = f"{romanization}.mp3"

        output_path = output_dir / filename
        segment.export(output_path, format="mp3")
        output_files.append(output_path)
        segments_saved += 1

        # Store timestamp info
        timestamps[romanization] = SegmentTimestamp(
            start_ms=start_ms,
            end_ms=end_ms,
            padded_start_ms=padded_start,
            padded_end_ms=padded_end,
        )

    return SegmentResult(
        source_file=audio_path,
        syllables=syllables,
        segments_found=len(boundaries) + skipped_segments,  # Original count before override
        segments_saved=segments_saved,
        output_files=output_files,
        source_label=source_label,
        mismatch=mismatch,
        override_applied=override_applied,
        skipped_segments=skipped_segments,
        effective_params={
            "min_silence": effective_min_silence,
            "threshold": effective_threshold,
            "padding": effective_padding,
            **({"skip_first": override["skip_first"]} if "skip_first" in override else {}),
            **({"skip_last": override["skip_last"]} if "skip_last" in override else {}),
        },
        timestamps=timestamps,
    )


ProgressCallback = Callable[["SegmentResult"], None]


def segment_lesson1(
    lesson_dir: Path,
    progress_callback: ProgressCallback | None = None,
    min_silence_len: int = 200,
    silence_thresh: int = -40,
    padding_ms: int = 50,
    reset_params: bool = False,
) -> dict[str, SegmentResult]:
    """Segment all row/column audio files from a lesson.

    Args:
        lesson_dir: Directory containing the scraped lesson files.
        progress_callback: Optional callback(result: SegmentResult).
        min_silence_len: Minimum silence duration (ms) to split on.
        silence_thresh: dBFS threshold for silence detection.
        padding_ms: Padding to add before/after each segment.
        reset_params: If True, ignore saved manifest params and use CLI values.

    Returns:
        Dict mapping source file to SegmentResult.
    """
    manifest_path = lesson_dir / "manifest.json"
    if not manifest_path.exists():
        raise FileNotFoundError(f"No manifest found at {manifest_path}")

    with open(manifest_path, encoding="utf-8") as f:
        manifest = json.load(f)

    syllables_dir = lesson_dir / "syllables"
    syllables_dir.mkdir(exist_ok=True)

    results = {}
    all_timestamps: dict[str, SegmentTimestamp] = {}

    # Process column audio (vowel columns) - only lesson1 has these
    columns = manifest.get("columns", {})
    for char, info in columns.items():
        audio_path = lesson_dir / info["file"]
        if not audio_path.exists():
            continue

        # Get per-source segment_params from manifest (unless reset)
        manifest_params = {} if reset_params else info.get("segment_params", {})

        source_label = f"col:{char} ({info.get('romanization', '?')})"
        result = segment_audio_file(
            audio_path=audio_path,
            syllables=info["syllables"],
            output_dir=syllables_dir,
            min_silence_len=min_silence_len,
            silence_thresh=silence_thresh,
            padding_ms=padding_ms,
            source_label=source_label,
            manifest_params=manifest_params,
        )
        results[str(audio_path)] = result
        all_timestamps.update(result.timestamps)

        # Store effective params back to manifest
        info["segment_params"] = result.effective_params

        if progress_callback:
            progress_callback(result)

    # Process row audio (consonant rows)
    rows = manifest.get("rows", {})
    for char, info in rows.items():
        audio_path = lesson_dir / info["file"]
        if not audio_path.exists():
            continue

        # Get per-source segment_params from manifest (unless reset)
        manifest_params = {} if reset_params else info.get("segment_params", {})

        source_label = f"row:{char} ({info.get('romanization', '?')})"
        result = segment_audio_file(
            audio_path=audio_path,
            syllables=info["syllables"],
            output_dir=syllables_dir,
            min_silence_len=min_silence_len,
            silence_thresh=silence_thresh,
            padding_ms=padding_ms,
            source_label=source_label,
            manifest_params=manifest_params,
        )
        results[str(audio_path)] = result
        all_timestamps.update(result.timestamps)

        # Store effective params back to manifest
        info["segment_params"] = result.effective_params

        if progress_callback:
            progress_callback(result)

    # Save updated manifest with segment_params
    with open(manifest_path, "w", encoding="utf-8") as f:
        json.dump(manifest, f, ensure_ascii=False, indent=2)

    # Update manifest with segment file info and timestamps
    update_manifest_with_segments(manifest_path, syllables_dir, all_timestamps)

    return results


def segment_single_row(
    lesson_dir: Path,
    row_romanization: str,
    min_silence: int = 200,
    threshold: int = -40,
    padding: int = 50,
    skip_first: int = 0,
    skip_last: int = 0,
) -> SegmentResult | None:
    """Re-segment a single row with custom parameters.

    Args:
        lesson_dir: Directory containing the lesson files.
        row_romanization: Romanization of the row (e.g., "b", "d", "k").
        min_silence: Minimum silence duration (ms) to split on.
        threshold: Silence threshold in dBFS.
        padding: Padding (ms) to add before/after each segment.
        skip_first: Number of segments to skip from start.
        skip_last: Number of segments to skip from end.

    Returns:
        SegmentResult if successful, None if row not found.
    """
    manifest_path = lesson_dir / "manifest.json"
    if not manifest_path.exists():
        raise FileNotFoundError(f"No manifest found at {manifest_path}")

    with open(manifest_path, encoding="utf-8") as f:
        manifest = json.load(f)

    syllables_dir = lesson_dir / "syllables"
    syllables_dir.mkdir(exist_ok=True)

    # Find the row by romanization
    rows = manifest.get("rows", {})
    target_row = None
    target_char = None

    for char, info in rows.items():
        if info.get("romanization") == row_romanization:
            target_row = info
            target_char = char
            break

    if not target_row:
        return None

    audio_path = lesson_dir / target_row["file"]
    if not audio_path.exists():
        return None

    # Build custom params
    custom_params = {
        "min_silence": min_silence,
        "threshold": threshold,
        "padding": padding,
    }
    if skip_first > 0:
        custom_params["skip_first"] = skip_first
    if skip_last > 0:
        custom_params["skip_last"] = skip_last

    source_label = f"row:{target_char} ({row_romanization})"
    result = segment_audio_file(
        audio_path=audio_path,
        syllables=target_row["syllables"],
        output_dir=syllables_dir,
        min_silence_len=200,  # Base defaults
        silence_thresh=-40,
        padding_ms=50,
        source_label=source_label,
        manifest_params=custom_params,
    )

    # Store effective params back to manifest
    target_row["segment_params"] = result.effective_params

    # Save updated manifest
    with open(manifest_path, "w", encoding="utf-8") as f:
        json.dump(manifest, f, ensure_ascii=False, indent=2)

    # Update segment file references and timestamps
    update_manifest_with_segments(manifest_path, syllables_dir, result.timestamps)

    return result


def update_manifest_with_segments(
    manifest_path: Path,
    syllables_dir: Path,
    all_timestamps: dict[str, SegmentTimestamp] | None = None,
) -> None:
    """Update the manifest with paths to segmented syllables and timestamps.

    IMPORTANT: Only updates syllables that are in all_timestamps.
    Syllables not in all_timestamps are left completely unchanged to preserve
    existing baselines and manual overrides from previous segmentation runs.

    Args:
        manifest_path: Path to the manifest.json file.
        syllables_dir: Directory containing segmented syllable files.
        all_timestamps: Dict mapping romanization -> SegmentTimestamp from segmentation.
                       Only syllables in this dict will have their segment info updated.
    """
    with open(manifest_path, encoding="utf-8") as f:
        manifest = json.load(f)

    syllable_table = manifest.get("syllable_table", {})
    all_timestamps = all_timestamps or {}

    # Only update syllables that were actually segmented in this run
    # Leave all others unchanged to preserve existing baselines and manual overrides
    for syllable, info in syllable_table.items():
        romanization = info.get("romanization", "")

        # CRITICAL: Only update if this syllable was part of the current segmentation
        if romanization not in all_timestamps:
            continue

        segment_file = f"syllables/{romanization}.mp3"
        full_path = syllables_dir / f"{romanization}.mp3"

        if full_path.exists():
            ts = all_timestamps[romanization]

            # Build new segment info with baseline timestamps
            segment_info: dict = {
                "file": segment_file,
                "baseline": {
                    "start_ms": ts.start_ms,
                    "end_ms": ts.end_ms,
                    "padded_start_ms": ts.padded_start_ms,
                    "padded_end_ms": ts.padded_end_ms,
                },
            }

            # Preserve existing manual overrides if any
            existing_segment = info.get("segment")
            if isinstance(existing_segment, dict) and "manual" in existing_segment:
                segment_info["manual"] = existing_segment["manual"]

            info["segment"] = segment_info

    with open(manifest_path, "w", encoding="utf-8") as f:
        json.dump(manifest, f, ensure_ascii=False, indent=2)


def apply_manual_segment(
    lesson_dir: Path,
    syllable: str,
    start_ms: int,
    end_ms: int,
    padding_ms: int = 75,
) -> bool:
    """Apply manual timestamp adjustment for a single syllable.

    Re-extracts the syllable audio using the specified timestamps and
    stores the manual override in the manifest.

    Args:
        lesson_dir: Directory containing the lesson files.
        syllable: The Korean syllable character to adjust.
        start_ms: Manual start time in milliseconds (relative to source row audio).
        end_ms: Manual end time in milliseconds.
        padding_ms: Padding to add before/after the segment.

    Returns:
        True if successful, False if syllable not found.
    """
    manifest_path = lesson_dir / "manifest.json"
    if not manifest_path.exists():
        raise FileNotFoundError(f"No manifest found at {manifest_path}")

    with open(manifest_path, encoding="utf-8") as f:
        manifest = json.load(f)

    syllable_table = manifest.get("syllable_table", {})
    syllable_info = syllable_table.get(syllable)
    if not syllable_info:
        return False

    romanization = syllable_info.get("romanization", "")

    # Find which row this syllable belongs to (for the source audio)
    rows = manifest.get("rows", {})
    source_row = None
    for char, row_info in rows.items():
        if syllable in row_info.get("syllables", []):
            source_row = row_info
            break

    if not source_row:
        return False

    # Load the source row audio
    audio_path = lesson_dir / source_row["file"]
    if not audio_path.exists():
        return False

    audio = AudioSegment.from_mp3(audio_path)

    # Apply padding but don't go out of bounds
    padded_start = max(0, start_ms - padding_ms)
    padded_end = min(len(audio), end_ms + padding_ms)

    # Extract segment
    segment = audio[padded_start:padded_end]

    # Save to syllables directory
    syllables_dir = lesson_dir / "syllables"
    syllables_dir.mkdir(exist_ok=True)
    output_path = syllables_dir / f"{romanization}.mp3"
    segment.export(output_path, format="mp3")

    # Update manifest with manual override (preserve baseline)
    segment_info = syllable_info.get("segment")
    if not isinstance(segment_info, dict) or not segment_info:
        # No existing segment info - create new with file path
        segment_info = {"file": f"syllables/{romanization}.mp3"}
    elif "file" not in segment_info:
        # Ensure file key exists even if segment was partial
        segment_info["file"] = f"syllables/{romanization}.mp3"

    segment_info["manual"] = {
        "start_ms": start_ms,
        "end_ms": end_ms,
        "padded_start_ms": padded_start,
        "padded_end_ms": padded_end,
    }
    syllable_info["segment"] = segment_info

    with open(manifest_path, "w", encoding="utf-8") as f:
        json.dump(manifest, f, ensure_ascii=False, indent=2)

    return True


def reset_manual_segment(lesson_dir: Path, syllable: str) -> bool:
    """Reset manual timestamp adjustment, restoring baseline.

    Re-extracts the syllable audio using the baseline timestamps and
    removes the manual override from the manifest.

    Args:
        lesson_dir: Directory containing the lesson files.
        syllable: The Korean syllable character to reset.

    Returns:
        True if successful, False if syllable not found or no baseline exists.
    """
    manifest_path = lesson_dir / "manifest.json"
    if not manifest_path.exists():
        raise FileNotFoundError(f"No manifest found at {manifest_path}")

    with open(manifest_path, encoding="utf-8") as f:
        manifest = json.load(f)

    syllable_table = manifest.get("syllable_table", {})
    syllable_info = syllable_table.get(syllable)
    if not syllable_info:
        return False

    romanization = syllable_info.get("romanization", "")
    segment_info = syllable_info.get("segment")

    if not isinstance(segment_info, dict):
        return False

    # Get baseline timestamps
    baseline = segment_info.get("baseline")
    if not baseline:
        # No baseline to restore to
        return False

    # Find which row this syllable belongs to (for the source audio)
    rows = manifest.get("rows", {})
    source_row = None
    for char, row_info in rows.items():
        if syllable in row_info.get("syllables", []):
            source_row = row_info
            break

    if not source_row:
        return False

    # Load the source row audio
    audio_path = lesson_dir / source_row["file"]
    if not audio_path.exists():
        return False

    audio = AudioSegment.from_mp3(audio_path)

    # Use baseline padded timestamps
    padded_start = baseline["padded_start_ms"]
    padded_end = baseline["padded_end_ms"]

    # Extract segment using baseline
    segment = audio[padded_start:padded_end]

    # Save to syllables directory
    syllables_dir = lesson_dir / "syllables"
    output_path = syllables_dir / f"{romanization}.mp3"
    segment.export(output_path, format="mp3")

    # Remove manual override from manifest
    if "manual" in segment_info:
        del segment_info["manual"]

    with open(manifest_path, "w", encoding="utf-8") as f:
        json.dump(manifest, f, ensure_ascii=False, indent=2)

    return True
