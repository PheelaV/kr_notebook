"""Test vocabulary loading from directory format.

This test ensures the vocabulary library supports the vocabulary/ directory
format with per-lesson files (vocabulary/lesson_01.json, etc.) in addition
to the legacy single vocabulary.json file format.
"""

import pytest

from conftest import TestClient


def test_vocabulary_directory_format_loads(admin_client: TestClient):
    """Vocabulary should load from vocabulary/ directory with lesson_*.json files."""
    # Enable the directory-format pack (admin can enable packs)
    response = admin_client.post("/settings/pack/test_vocab_directory_pack/enable")
    assert response.status_code in (200, 303), f"Failed to enable pack: {response.status_code}"

    # Try to load vocabulary page
    response = admin_client.get("/library/vocabulary")
    assert response.status_code == 200

    # Should NOT show "Pack Not Enabled" message
    assert "Vocabulary Pack Not Enabled" not in response.text, (
        "Vocabulary page shows 'Pack Not Enabled' - directory format not supported"
    )

    # Should show vocabulary content from the directory format pack
    assert "테스트" in response.text or "te-seu-teu" in response.text, (
        "Vocabulary content from directory format pack not found in page"
    )
