"""Scraper for Lesson 3 diphthongs and combined vowels from howtostudykorean.com.

Lesson 3 introduces:
- Combined vowels: ㅐ (ae), ㅔ (e), ㅒ (yae), ㅖ (ye)
- Diphthongs: ㅘ (wa), ㅙ (wae), ㅚ (oe), ㅝ (wo), ㅞ (we), ㅟ (wi), ㅢ (ui)

Unlike Lessons 1-2 which have row audio for each consonant,
Lesson 3 has row audio organized by VOWEL, with varying numbers of
example syllables per vowel.
"""

import json
from dataclasses import dataclass, field
from datetime import datetime, timezone
from pathlib import Path
from typing import Callable

from .utils import download_file

LESSON3_URL = "https://www.howtostudykorean.com/unit0/unit-0-lesson-3/"

# Base URL for audio files
AUDIO_BASE_URL = "https://www.howtostudykorean.com/wp-content/uploads/2014/08/"
AUDIO_BASE_URL_ALT = "https://www.howtostudykorean.com/wp-content/uploads/2016/01/"

# Vowel audio definitions with their example syllables
# Each audio file demonstrates a vowel paired with various consonants
LESSON3_VOWELS = {
    # Combined vowels (ae, e, yae, ye)
    "ㅐ": {
        "romanization": "ae",
        "file": "Unit0Lesson31.mp3",
        "base_url": AUDIO_BASE_URL,
        "syllables": ["애", "배", "재", "대", "개", "새", "매", "내", "해", "래"],
    },
    # ㅔ sounds identical to ㅐ - same audio file used
    "ㅔ": {
        "romanization": "e",
        "file": "Unit0Lesson31.mp3",  # Same as ㅐ
        "base_url": AUDIO_BASE_URL,
        "syllables": ["에", "베", "제", "데", "게", "세", "메", "네", "헤", "레"],
        "shares_audio_with": "ㅐ",
    },
    "ㅒ": {
        "romanization": "yae",
        "file": "Unit0Lesson39.mp3",
        "base_url": AUDIO_BASE_URL,
        "syllables": ["얘"],
    },
    "ㅖ": {
        "romanization": "ye",
        "file": "Unit0Lesson37.mp3",
        "base_url": AUDIO_BASE_URL,
        "syllables": ["예", "계", "혜"],
    },
    # Diphthongs (wa, wae, oe, wo, we, wi, ui)
    "ㅘ": {
        "romanization": "wa",
        "file": "Unit0Lesson35.mp3",
        "base_url": AUDIO_BASE_URL,
        "syllables": ["와", "봐", "좌", "돠", "과", "솨", "놔"],
    },
    "ㅙ": {
        "romanization": "wae",
        "file": "Unit0Lesson38.mp3",
        "base_url": AUDIO_BASE_URL,
        "syllables": ["왜"],
    },
    "ㅚ": {
        "romanization": "oe",
        "file": "Unit0Lesson34.mp3",
        "base_url": AUDIO_BASE_URL,
        "syllables": ["외", "뵈", "죄", "되", "괴", "쇠", "뇌"],
    },
    "ㅝ": {
        "romanization": "wo",
        "file": "Unit0Lesson33.mp3",
        "base_url": AUDIO_BASE_URL,
        "syllables": ["워", "붜", "줘", "둬", "궈", "숴", "눠"],
    },
    "ㅞ": {
        "romanization": "we",
        "file": "Unit0Lesson310.mp3",
        "base_url": AUDIO_BASE_URL,
        "syllables": ["웨"],
    },
    "ㅟ": {
        "romanization": "wi",
        "file": "Unit0Lesson32.mp3",
        "base_url": AUDIO_BASE_URL,
        "syllables": ["위", "뷔", "쥐", "뒤", "귀", "쉬", "뉘"],
    },
    "ㅢ": {
        "romanization": "ui",
        "file": "Unit0Pron1.mp3",
        "base_url": AUDIO_BASE_URL_ALT,
        "syllables": ["의", "븨", "즤", "듸", "긔", "싀", "늬"],
    },
}

# Order of vowels for display (combined vowels first, then diphthongs)
LESSON3_VOWELS_ORDER = ["ㅐ", "ㅔ", "ㅒ", "ㅖ", "ㅘ", "ㅙ", "ㅚ", "ㅝ", "ㅞ", "ㅟ", "ㅢ"]

# Vowels without audio (Y-vowels mentioned but no dedicated audio)
LESSON3_VOWELS_NO_AUDIO = ["ㅑ", "ㅕ", "ㅠ", "ㅛ"]

# Romanization for syllables - maps consonant to romanization prefix
CONSONANT_ROMANIZATION = {
    "ㅇ": "",  # Silent initial
    "ㅂ": "b",
    "ㅈ": "j",
    "ㄷ": "d",
    "ㄱ": "g",
    "ㅅ": "s",
    "ㅁ": "m",
    "ㄴ": "n",
    "ㅎ": "h",
    "ㄹ": "r",
    "ㄲ": "kk",
    "ㅃ": "pp",
    "ㅉ": "jj",
    "ㄸ": "tt",
    "ㅆ": "ss",
    "ㅋ": "k",
    "ㅍ": "p",
    "ㅊ": "ch",
    "ㅌ": "t",
    "ㄱ": "g",
    "ㄴ": "n",
    "ㅃ": "pp",
    "ㅉ": "jj",
    "ㅊ": "ch",
    "ㅋ": "k",
    "ㅌ": "t",
    "ㅍ": "p",
    "ㄲ": "kk",
    "ㄸ": "tt",
    "ㅆ": "ss",
    "ㅎ": "h",
    "ㄹ": "l",
    "ㅁ": "m",
    "ㅅ": "s",
    "ㅈ": "j",
    "ㄷ": "d",
    "ㅂ": "b",
}


def decompose_syllable(syllable: str) -> tuple[str, str]:
    """Decompose a Korean syllable into consonant and vowel.

    Returns tuple of (consonant, vowel) jamo.
    """
    code = ord(syllable) - 0xAC00
    if code < 0 or code > 11171:
        return ("", "")

    # Hangul composition: code = cho*588 + jung*28 + jong
    cho = code // 588
    jung = (code % 588) // 28

    CHOSEONG = [
        "ㄱ", "ㄲ", "ㄴ", "ㄷ", "ㄸ", "ㄹ", "ㅁ", "ㅂ", "ㅃ", "ㅅ",
        "ㅆ", "ㅇ", "ㅈ", "ㅉ", "ㅊ", "ㅋ", "ㅌ", "ㅍ", "ㅎ",
    ]
    JUNGSEONG = [
        "ㅏ", "ㅐ", "ㅑ", "ㅒ", "ㅓ", "ㅔ", "ㅕ", "ㅖ", "ㅗ", "ㅘ",
        "ㅙ", "ㅚ", "ㅛ", "ㅜ", "ㅝ", "ㅞ", "ㅟ", "ㅠ", "ㅡ", "ㅢ",
        "ㅣ",
    ]

    return (CHOSEONG[cho], JUNGSEONG[jung])


def romanize_syllable(syllable: str, vowel_rom: str) -> str:
    """Get romanization for a syllable.

    Args:
        syllable: Korean syllable (e.g., "배")
        vowel_rom: Romanization of the vowel (e.g., "ae")

    Returns:
        Romanized syllable (e.g., "bae")
    """
    consonant, _ = decompose_syllable(syllable)

    c_rom = CONSONANT_ROMANIZATION.get(consonant, "")
    return f"{c_rom}{vowel_rom}"


@dataclass
class VowelRow:
    """Represents a vowel row audio file from Lesson 3.

    Each audio file plays examples of a vowel paired with various consonants.
    """
    vowel: str
    url: str
    romanization: str
    filename: str
    syllables: list[str] = field(default_factory=list)
    shares_audio_with: str | None = None


def get_lesson3_audio_files() -> list[VowelRow]:
    """Get the list of vowel row audio files for Lesson 3.

    Returns:
        List of VowelRow objects.
    """
    audio_files = []

    for vowel in LESSON3_VOWELS_ORDER:
        info = LESSON3_VOWELS[vowel]

        audio_files.append(
            VowelRow(
                vowel=vowel,
                url=f"{info['base_url']}{info['file']}",
                romanization=info["romanization"],
                filename=f"row_{info['romanization']}.mp3",
                syllables=info["syllables"],
                shares_audio_with=info.get("shares_audio_with"),
            )
        )

    return audio_files


def create_manifest(
    audio_files: list[VowelRow],
    downloaded: dict[str, bool],
    existing_manifest: dict | None = None,
) -> dict:
    """Create a manifest JSON structure for Lesson 3, preserving existing segment_params.

    Args:
        audio_files: List of VowelRow objects.
        downloaded: Dict mapping vowel to download success status.
        existing_manifest: Optional existing manifest to preserve segment_params from.

    Returns:
        Manifest dictionary ready for JSON serialization.
    """
    rows = {}

    for af in audio_files:
        if not downloaded.get(af.vowel, False):
            continue

        row_data = {
            "file": f"rows/{af.filename}",
            "romanization": af.romanization,
            "source_url": af.url,
            "syllables": af.syllables,
        }

        if af.shares_audio_with:
            row_data["shares_audio_with"] = af.shares_audio_with

        rows[af.vowel] = row_data

    # Preserve segment_params from existing manifest
    if existing_manifest:
        for vowel, info in existing_manifest.get("rows", {}).items():
            if vowel in rows and "segment_params" in info:
                rows[vowel]["segment_params"] = info["segment_params"]

    # Build syllable table for all syllables in the audio
    syllable_table = {}
    for vowel in LESSON3_VOWELS_ORDER:
        info = LESSON3_VOWELS[vowel]
        v_rom = info["romanization"]

        for syllable in info["syllables"]:
            consonant, _ = decompose_syllable(syllable)
            rom = romanize_syllable(syllable, v_rom)

            syllable_table[syllable] = {
                "consonant": consonant,
                "vowel": vowel,
                "romanization": rom,
                "segment_file": None,
            }

    # Preserve segment_file from existing manifest
    if existing_manifest:
        for syllable, info in existing_manifest.get("syllable_table", {}).items():
            if syllable in syllable_table and info.get("segment_file"):
                syllable_table[syllable]["segment_file"] = info["segment_file"]

    return {
        "source": "howtostudykorean.com",
        "lesson": "unit0/lesson3",
        "scraped_at": datetime.now(timezone.utc).isoformat(),
        "rows": rows,
        "syllable_table": syllable_table,
        "vowels_order": LESSON3_VOWELS_ORDER,
        "vowels_no_audio": LESSON3_VOWELS_NO_AUDIO,
    }


ProgressCallback = Callable[[int, int, str, bool], None]


def scrape_lesson3(
    output_dir: Path,
    progress_callback: ProgressCallback | None = None,
    skip_existing: bool = True,
) -> dict:
    """Scrape vowel pronunciation audio from Lesson 3.

    Directory structure:
        output_dir/
        ├── rows/                 # Vowel row audio files
        │   ├── row_ae.mp3        # ㅐ row: 애 배 재 대 개 새 매 내 해 래
        │   ├── row_wi.mp3        # ㅟ row: 위 뷔 쥐 뒤 귀 쉬 뉘
        │   └── ...
        ├── syllables/            # Individual syllables (created by segmentation)
        │   └── (empty until segment command is run)
        └── manifest.json

    Args:
        output_dir: Directory to save audio files and manifest.
        progress_callback: Optional callback(current, total, vowel, success).
        skip_existing: Skip files that already exist.

    Returns:
        The manifest dictionary.
    """
    audio_files = get_lesson3_audio_files()

    if not audio_files:
        raise ValueError("No audio files defined for Lesson 3.")

    # Ensure output directories exist
    output_dir.mkdir(parents=True, exist_ok=True)
    (output_dir / "rows").mkdir(exist_ok=True)
    (output_dir / "syllables").mkdir(exist_ok=True)

    # Track which audio files we've already downloaded (for shared audio)
    downloaded_urls: dict[str, Path] = {}

    # Download each file
    downloaded: dict[str, bool] = {}
    total = len(audio_files)

    for i, af in enumerate(audio_files, 1):
        output_path = output_dir / "rows" / af.filename

        if skip_existing and output_path.exists():
            downloaded[af.vowel] = True
            if progress_callback:
                progress_callback(i, total, af.vowel, True)
            continue

        # Check if this audio is shared with another vowel
        if af.shares_audio_with and af.url in downloaded_urls:
            # Copy the already-downloaded file
            import shutil
            src_path = downloaded_urls[af.url]
            if src_path.exists():
                shutil.copy(src_path, output_path)
                downloaded[af.vowel] = True
                if progress_callback:
                    progress_callback(i, total, af.vowel, True)
                continue

        success = download_file(af.url, output_path)
        downloaded[af.vowel] = success

        if success:
            downloaded_urls[af.url] = output_path

        if progress_callback:
            progress_callback(i, total, af.vowel, success)

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
