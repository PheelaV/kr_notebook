"""Project path constants - single source of truth for all file paths.

This module centralizes path definitions to avoid fragile parent traversal
and hardcoded strings scattered throughout the codebase.
"""

from pathlib import Path

import pyrootutils

# Project root (contains Cargo.toml)
PROJECT_ROOT = pyrootutils.find_root(search_from=__file__, indicator="Cargo.toml")

# Data directories
DATA_DIR = PROJECT_ROOT / "data"
SCRAPED_DIR = DATA_DIR / "scraped"
HTSK_DIR = SCRAPED_DIR / "htsk"


def lesson_dir(lesson: str) -> Path:
    """Get the lesson directory path."""
    return HTSK_DIR / lesson


def manifest_path(lesson: str) -> Path:
    """Get the manifest.json path for a lesson."""
    return HTSK_DIR / lesson / "manifest.json"


def syllables_dir(lesson: str) -> Path:
    """Get the syllables directory path for a lesson."""
    return HTSK_DIR / lesson / "syllables"


def rows_dir(lesson: str) -> Path:
    """Get the rows directory path for a lesson."""
    return HTSK_DIR / lesson / "rows"


def columns_dir(lesson: str) -> Path:
    """Get the columns directory path for a lesson."""
    return HTSK_DIR / lesson / "columns"
