"""Scraper for Lesson 1 pronunciation table from howtostudykorean.com."""

import json
import re
from dataclasses import dataclass, field
from datetime import datetime, timezone
from pathlib import Path
from typing import Callable

from .utils import download_file, fetch_page, parse_html

LESSON1_URL = "https://www.howtostudykorean.com/unit0/unit0lesson1/"

# Character romanization mapping
ROMANIZATION = {
    # Vowels
    "ㅣ": "i",
    "ㅏ": "a",
    "ㅓ": "eo",
    "ㅡ": "eu",
    "ㅜ": "u",
    "ㅗ": "o",
    # Consonants
    "ㅂ": "b",
    "ㅈ": "j",
    "ㄷ": "d",
    "ㄱ": "g",
    "ㅅ": "s",
    "ㅁ": "m",
    "ㄴ": "n",
    "ㅎ": "h",
    "ㄹ": "r",
}

# Ordered lists for table structure
VOWELS_ORDER = ["ㅣ", "ㅏ", "ㅓ", "ㅡ", "ㅜ", "ㅗ"]
CONSONANTS_ORDER = ["ㅂ", "ㅈ", "ㄷ", "ㄱ", "ㅅ", "ㅁ", "ㄴ", "ㅎ", "ㄹ"]

# Classify characters
VOWELS = set(VOWELS_ORDER)
CONSONANTS = set(CONSONANTS_ORDER)

# Combined syllables in the table (consonant + vowel)
# Row = consonant, Column = vowel
SYLLABLE_TABLE = {
    (c, v): chr(
        0xAC00
        + (CONSONANTS_ORDER.index(c) if c in CONSONANTS else 0) * 588
        + VOWELS_ORDER.index(v) * 28
    )
    for c in CONSONANTS_ORDER
    for v in VOWELS_ORDER
}

# Actually compute correctly using proper Hangul composition
def compose_syllable(consonant: str, vowel: str) -> str:
    """Compose a Korean syllable from consonant + vowel."""
    # Mapping to Hangul Jamo initial consonants (Choseong)
    CHOSEONG = {
        "ㄱ": 0, "ㄲ": 1, "ㄴ": 2, "ㄷ": 3, "ㄸ": 4,
        "ㄹ": 5, "ㅁ": 6, "ㅂ": 7, "ㅃ": 8, "ㅅ": 9,
        "ㅆ": 10, "ㅇ": 11, "ㅈ": 12, "ㅉ": 13, "ㅊ": 14,
        "ㅋ": 15, "ㅌ": 16, "ㅍ": 17, "ㅎ": 18,
    }
    # Mapping to Hangul Jamo vowels (Jungseong)
    JUNGSEONG = {
        "ㅏ": 0, "ㅐ": 1, "ㅑ": 2, "ㅒ": 3, "ㅓ": 4,
        "ㅔ": 5, "ㅕ": 6, "ㅖ": 7, "ㅗ": 8, "ㅘ": 9,
        "ㅙ": 10, "ㅚ": 11, "ㅛ": 12, "ㅜ": 13, "ㅝ": 14,
        "ㅞ": 15, "ㅟ": 16, "ㅠ": 17, "ㅡ": 18, "ㅢ": 19,
        "ㅣ": 20,
    }

    cho = CHOSEONG.get(consonant, 0)
    jung = JUNGSEONG.get(vowel, 0)
    # No final consonant (jongseong = 0)
    return chr(0xAC00 + cho * 588 + jung * 28)


@dataclass
class AudioFile:
    """Represents a scraped audio file.

    For Lesson 1 pronunciation table:
    - Column audio (vowel headers): Contains the vowel + all consonant-vowel combos
      e.g., ㅣ audio = [ㅣ, 비, 지, 디, 기, 시, 미, 니, 히, 리]
    - Row audio (consonant first column): Contains consonant + all vowel combos
      e.g., ㅂ audio = [ㅂ, 비, 바, 버, 브, 부, 보]
    """

    character: str
    url: str
    audio_type: str  # "column" (vowel) or "row" (consonant)
    romanization: str
    filename: str
    # The syllables contained in this audio, in order
    syllables: list[str] = field(default_factory=list)


def parse_pronunciation_table(html: str) -> list[AudioFile]:
    """Parse the pronunciation table from the lesson HTML.

    Extracts audio URLs from anchor tags that link to MP3 files.
    The character is the text content of the anchor tag.

    Audio structure:
    - Vowel headers (first row): Each plays the entire column (vowel + all C+V combos)
    - Consonant headers (first column): Each plays the entire row (consonant + all C+V combos)

    Args:
        html: The raw HTML of the lesson page.

    Returns:
        List of AudioFile objects with character/URL mappings.
    """
    soup = parse_html(html)
    audio_files = []

    # Find all anchor tags with MP3 links
    for anchor in soup.find_all("a", href=re.compile(r"\.mp3$", re.IGNORECASE)):
        url = anchor.get("href", "")
        # Get the text content (the Korean character)
        char = anchor.get_text(strip=True)

        # Skip if not a single Korean character we recognize
        if char not in ROMANIZATION:
            continue

        romanization = ROMANIZATION[char]

        if char in VOWELS:
            # Column audio: plays down the column (consonant+vowel for each consonant)
            # e.g., ㅣ column plays: 비, 지, 디, 기, 시, 미, 니, 히, 리
            audio_type = "column"
            filename = f"col_{romanization}.mp3"
            syllables = [compose_syllable(c, char) for c in CONSONANTS_ORDER]
        else:
            # Row audio: plays across the row (consonant+vowel for each vowel)
            # e.g., ㅂ row plays: 비, 바, 버, 브, 부, 보
            audio_type = "row"
            filename = f"row_{romanization}.mp3"
            syllables = [compose_syllable(char, v) for v in VOWELS_ORDER]

        audio_files.append(
            AudioFile(
                character=char,
                url=url,
                audio_type=audio_type,
                romanization=romanization,
                filename=filename,
                syllables=syllables,
            )
        )

    return audio_files


def create_manifest(
    audio_files: list[AudioFile],
    downloaded: dict[str, bool],
    existing_manifest: dict | None = None,
) -> dict:
    """Create a manifest JSON structure, preserving existing segment_params.

    Args:
        audio_files: List of AudioFile objects.
        downloaded: Dict mapping character to download success status.
        existing_manifest: Optional existing manifest to preserve segment_params from.

    Returns:
        Manifest dictionary ready for JSON serialization.
    """
    columns = {}
    rows = {}

    for af in audio_files:
        if not downloaded.get(af.character, False):
            continue

        entry = {
            "file": f"{af.audio_type}s/{af.filename}",
            "romanization": af.romanization,
            "source_url": af.url,
            "syllables": af.syllables,
        }

        if af.audio_type == "column":
            columns[af.character] = entry
        else:
            rows[af.character] = entry

    # Preserve segment_params from existing manifest
    if existing_manifest:
        for char, info in existing_manifest.get("columns", {}).items():
            if char in columns and "segment_params" in info:
                columns[char]["segment_params"] = info["segment_params"]
        for char, info in existing_manifest.get("rows", {}).items():
            if char in rows and "segment_params" in info:
                rows[char]["segment_params"] = info["segment_params"]

    # Build complete syllable table
    syllable_table = {}
    for c in CONSONANTS_ORDER:
        for v in VOWELS_ORDER:
            syllable = compose_syllable(c, v)
            c_rom = ROMANIZATION[c]
            v_rom = ROMANIZATION[v]
            syllable_table[syllable] = {
                "consonant": c,
                "vowel": v,
                "romanization": f"{c_rom}{v_rom}",
                # Segment file will be created by audio segmentation
                "segment_file": None,
            }

    # Preserve segment_file from existing manifest
    if existing_manifest:
        for syllable, info in existing_manifest.get("syllable_table", {}).items():
            if syllable in syllable_table and info.get("segment_file"):
                syllable_table[syllable]["segment_file"] = info["segment_file"]

    return {
        "source": "howtostudykorean.com",
        "lesson": "unit0/lesson1",
        "scraped_at": datetime.now(timezone.utc).isoformat(),
        "columns": columns,  # Vowel column audio (each contains 10 syllables)
        "rows": rows,  # Consonant row audio (each contains 7 syllables)
        "syllable_table": syllable_table,  # 54 syllable combinations
        "vowels_order": VOWELS_ORDER,
        "consonants_order": CONSONANTS_ORDER,
    }


ProgressCallback = Callable[[int, int, str, bool], None]


def scrape_lesson1(
    output_dir: Path,
    progress_callback: ProgressCallback | None = None,
    skip_existing: bool = True,
) -> dict:
    """Scrape all pronunciation audio from Lesson 1.

    Directory structure:
        output_dir/
        ├── columns/          # Column audio (vowel + all C+V in that column)
        │   ├── col_i.mp3     # ㅣ column: ㅣ 비 지 디 기 시 미 니 히 리
        │   └── ...
        ├── rows/             # Row audio (consonant + all C+V in that row)
        │   ├── row_b.mp3     # ㅂ row: ㅂ 비 바 버 브 부 보
        │   └── ...
        ├── syllables/        # Individual syllables (created by segmentation)
        │   └── (empty until segment command is run)
        └── manifest.json

    Args:
        output_dir: Directory to save audio files and manifest.
        progress_callback: Optional callback(current, total, char, success).
        skip_existing: Skip files that already exist.

    Returns:
        The manifest dictionary.
    """
    # Fetch and parse the page
    html = fetch_page(LESSON1_URL)
    audio_files = parse_pronunciation_table(html)

    if not audio_files:
        raise ValueError("No audio files found on the page. Structure may have changed.")

    # Ensure output directories exist
    output_dir.mkdir(parents=True, exist_ok=True)
    (output_dir / "columns").mkdir(exist_ok=True)
    (output_dir / "rows").mkdir(exist_ok=True)
    (output_dir / "syllables").mkdir(exist_ok=True)

    # Download each file
    downloaded: dict[str, bool] = {}
    total = len(audio_files)

    for i, af in enumerate(audio_files, 1):
        # Put in appropriate subdirectory
        subdir = "columns" if af.audio_type == "column" else "rows"
        output_path = output_dir / subdir / af.filename

        if skip_existing and output_path.exists():
            downloaded[af.character] = True
            if progress_callback:
                progress_callback(i, total, af.character, True)
            continue

        success = download_file(af.url, output_path)
        downloaded[af.character] = success

        if progress_callback:
            progress_callback(i, total, af.character, success)

    # Load existing manifest if present (to preserve segment_params)
    manifest_path = output_dir / "manifest.json"
    existing_manifest = None
    if manifest_path.exists():
        with open(manifest_path, encoding="utf-8") as f:
            existing_manifest = json.load(f)

    # Create and save manifest, preserving segment_params from existing
    manifest = create_manifest(audio_files, downloaded, existing_manifest)
    with open(manifest_path, "w", encoding="utf-8") as f:
        json.dump(manifest, f, ensure_ascii=False, indent=2)

    return manifest
