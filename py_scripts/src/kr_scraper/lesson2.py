"""Scraper for Lesson 2 pronunciation from howtostudykorean.com.

Lesson 2 introduces:
- ㅇ (silent initial / ng final)
- Double/tense consonants: ㄲ, ㅃ, ㅉ, ㄸ, ㅆ
- Aspirated consonants: ㅋ, ㅍ, ㅊ, ㅌ

Each audio file plays a ROW of syllables (consonant + all 6 vowels),
just like Lesson 1 rows.
"""

import json
from dataclasses import dataclass, field
from datetime import datetime, timezone
from pathlib import Path
from typing import Callable

from .lesson1 import ROMANIZATION as LESSON1_ROMANIZATION, VOWELS_ORDER, compose_syllable
from .utils import download_file

LESSON2_URL = "https://www.howtostudykorean.com/unit0/unit-0-lesson-2/"

# New consonants introduced in Lesson 2 with their row audio files
# Each audio plays: consonant + vowel for all 6 vowels (이/아/어/으/우/오)
LESSON2_CONSONANTS = {
    # Silent initial / ng final
    "ㅇ": {"romanization": "ng", "file": "O.mp3"},
    # Double (tense) consonants
    "ㄲ": {"romanization": "kk", "file": "Kk.mp3"},
    "ㅃ": {"romanization": "pp", "file": "Pp.mp3"},
    "ㅉ": {"romanization": "jj", "file": "Jj.mp3"},
    "ㄸ": {"romanization": "tt", "file": "Dd.mp3"},
    "ㅆ": {"romanization": "ss", "file": "Ss.mp3"},
    # Aspirated consonants
    "ㅋ": {"romanization": "k", "file": "K1.mp3"},
    "ㅍ": {"romanization": "p", "file": "P.mp3"},
    "ㅊ": {"romanization": "ch", "file": "Ch.mp3"},
    "ㅌ": {"romanization": "t", "file": "T.mp3"},
}

# Order in the table (from top to bottom in lesson 2 table)
LESSON2_CONSONANTS_ORDER = ["ㅇ", "ㄲ", "ㅋ", "ㅃ", "ㅍ", "ㅉ", "ㅊ", "ㄸ", "ㅌ", "ㅆ"]

# Romanization for syllables (extend lesson 1 mappings)
ROMANIZATION = {
    **LESSON1_ROMANIZATION,
    "ㅇ": "ng",  # as initial it's silent, but we use 'ng' for filename
    "ㄲ": "kk",
    "ㅃ": "pp",
    "ㅉ": "jj",
    "ㄸ": "tt",
    "ㅆ": "ss",
    "ㅋ": "k",
    "ㅍ": "p",
    "ㅊ": "ch",
    "ㅌ": "t",
}

# Base URL for audio files
AUDIO_BASE_URL = "https://www.howtostudykorean.com/wp-content/uploads/2014/01/"


@dataclass
class RowAudio:
    """Represents a row audio file from Lesson 2.

    Each audio file plays the entire row: consonant + all 6 vowels.
    e.g., ㄲ audio = 끼 까 꺼 끄 꾸 꼬
    """

    character: str
    url: str
    romanization: str
    filename: str
    syllables: list[str] = field(default_factory=list)


def get_lesson2_audio_files() -> list[RowAudio]:
    """Get the list of row audio files for Lesson 2 consonants.

    Each audio plays the entire row (consonant + 6 vowels).

    Returns:
        List of RowAudio objects.
    """
    audio_files = []

    for char in LESSON2_CONSONANTS_ORDER:
        info = LESSON2_CONSONANTS[char]
        # Build syllables list for this row
        syllables = [compose_syllable(char, v) for v in VOWELS_ORDER]

        audio_files.append(
            RowAudio(
                character=char,
                url=f"{AUDIO_BASE_URL}{info['file']}",
                romanization=info["romanization"],
                filename=f"row_{info['romanization']}.mp3",
                syllables=syllables,
            )
        )

    return audio_files


def create_manifest(
    audio_files: list[RowAudio],
    downloaded: dict[str, bool],
) -> dict:
    """Create a manifest JSON structure for Lesson 2.

    Args:
        audio_files: List of RowAudio objects.
        downloaded: Dict mapping character to download success status.

    Returns:
        Manifest dictionary ready for JSON serialization.
    """
    rows = {}

    for af in audio_files:
        if not downloaded.get(af.character, False):
            continue

        rows[af.character] = {
            "file": f"rows/{af.filename}",
            "romanization": af.romanization,
            "source_url": af.url,
            "syllables": af.syllables,
        }

    # Build syllable table for lesson 2 consonants
    syllable_table = {}
    for c in LESSON2_CONSONANTS_ORDER:
        c_info = LESSON2_CONSONANTS[c]
        for v in VOWELS_ORDER:
            syllable = compose_syllable(c, v)
            v_rom = LESSON1_ROMANIZATION[v]

            # Special handling for ㅇ (silent as initial)
            if c == "ㅇ":
                rom = v_rom  # Just the vowel sound
            else:
                rom = f"{c_info['romanization']}{v_rom}"

            syllable_table[syllable] = {
                "consonant": c,
                "vowel": v,
                "romanization": rom,
                "segment_file": None,  # Will be set by segmentation
            }

    return {
        "source": "howtostudykorean.com",
        "lesson": "unit0/lesson2",
        "scraped_at": datetime.now(timezone.utc).isoformat(),
        "rows": rows,  # Row audio (each contains 6 syllables)
        "syllable_table": syllable_table,  # 60 syllable combinations (10 × 6)
        "vowels_order": VOWELS_ORDER,
        "consonants_order": LESSON2_CONSONANTS_ORDER,
    }


ProgressCallback = Callable[[int, int, str, bool], None]


def scrape_lesson2(
    output_dir: Path,
    progress_callback: ProgressCallback | None = None,
    skip_existing: bool = True,
) -> dict:
    """Scrape row pronunciation audio from Lesson 2.

    Directory structure:
        output_dir/
        ├── rows/                 # Row audio (each plays 6 syllables)
        │   ├── row_ng.mp3        # ㅇ row: 이 아 어 으 우 오
        │   ├── row_kk.mp3        # ㄲ row: 끼 까 꺼 끄 꾸 꼬
        │   └── ...
        ├── syllables/            # Individual syllables (created by segmentation)
        │   └── (empty until segment command is run)
        └── manifest.json

    Args:
        output_dir: Directory to save audio files and manifest.
        progress_callback: Optional callback(current, total, char, success).
        skip_existing: Skip files that already exist.

    Returns:
        The manifest dictionary.
    """
    audio_files = get_lesson2_audio_files()

    if not audio_files:
        raise ValueError("No audio files defined for Lesson 2.")

    # Ensure output directories exist
    output_dir.mkdir(parents=True, exist_ok=True)
    (output_dir / "rows").mkdir(exist_ok=True)
    (output_dir / "syllables").mkdir(exist_ok=True)

    # Download each file
    downloaded: dict[str, bool] = {}
    total = len(audio_files)

    for i, af in enumerate(audio_files, 1):
        output_path = output_dir / "rows" / af.filename

        if skip_existing and output_path.exists():
            downloaded[af.character] = True
            if progress_callback:
                progress_callback(i, total, af.character, True)
            continue

        success = download_file(af.url, output_path)
        downloaded[af.character] = success

        if progress_callback:
            progress_callback(i, total, af.character, success)

    # Create and save manifest
    manifest = create_manifest(audio_files, downloaded)
    manifest_path = output_dir / "manifest.json"
    with open(manifest_path, "w", encoding="utf-8") as f:
        json.dump(manifest, f, ensure_ascii=False, indent=2)

    return manifest
