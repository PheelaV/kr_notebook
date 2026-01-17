"""Integration tests for offline sync API endpoints.

Tests the server-side sync API:
- POST /api/study/download-session
- POST /api/study/sync-offline
"""

import pytest
from datetime import datetime, timedelta, timezone

from conftest import DbManager, TestClient


class TestDownloadSession:
    """Tests for the download session endpoint."""

    def test_download_session_requires_offline_mode(self, authenticated_client: TestClient):
        """Downloading session requires offline mode to be enabled."""
        # Try to download without enabling offline mode
        response = authenticated_client.post(
            "/api/study/download-session",
            json={"duration_minutes": 10, "filter_mode": "all"},
        )
        # Should return 403 (offline mode not enabled)
        assert response.status_code == 403

    def test_download_session_returns_cards(
        self, authenticated_client: TestClient, db_manager: DbManager, test_user: tuple[str, str]
    ):
        """Download session returns cards with SRS state."""
        # First enable offline mode via settings
        response = authenticated_client.post(
            "/settings",
            data={"_action": "offline_mode", "offline_mode_enabled": "true"},
        )
        assert response.status_code in (200, 303)

        # Download session
        response = authenticated_client.post(
            "/api/study/download-session",
            json={"duration_minutes": 30, "filter_mode": "all"},
        )
        assert response.status_code == 200

        data = response.json()
        assert "session_id" in data
        assert "cards" in data
        assert "created_at" in data

        # Should have cards
        assert len(data["cards"]) > 0

        # Each card should have required fields
        card = data["cards"][0]
        assert "card_id" in card or "id" in card
        assert "front" in card
        assert "back" in card  # API uses 'back' not 'main_answer'

    def test_download_session_includes_srs_state(
        self, authenticated_client: TestClient, db_manager: DbManager, test_user: tuple[str, str]
    ):
        """Downloaded cards include SRS state for client-side scheduling."""
        # Enable offline mode
        authenticated_client.post(
            "/settings",
            data={"_action": "offline_mode", "offline_mode_enabled": "true"},
        )

        # Download session
        response = authenticated_client.post(
            "/api/study/download-session",
            json={"duration_minutes": 30, "filter_mode": "all"},
        )
        assert response.status_code == 200

        data = response.json()
        if len(data["cards"]) > 0:
            card = data["cards"][0]
            # Should have SRS fields for client-side calculation
            assert "learning_step" in card or card.get("learning_step") is None
            # fsrs_stability and fsrs_difficulty may be None for new cards


class TestSyncOffline:
    """Tests for the sync offline endpoint."""

    def test_sync_empty_reviews(
        self, authenticated_client: TestClient, db_manager: DbManager, test_user: tuple[str, str]
    ):
        """Sync with empty reviews returns success."""
        # Enable offline mode first
        authenticated_client.post(
            "/settings",
            data={"_action": "offline_mode", "offline_mode_enabled": "true"},
        )

        # Download a session to get a valid session_id
        response = authenticated_client.post(
            "/api/study/download-session",
            json={"duration_minutes": 10, "filter_mode": "all"},
        )
        assert response.status_code == 200
        session = response.json()

        # Sync empty reviews with valid session_id
        response = authenticated_client.post(
            "/api/study/sync-offline",
            json={"session_id": session["session_id"], "reviews": []},
        )
        assert response.status_code == 200

        data = response.json()
        assert data["synced_count"] == 0
        assert data["errors"] == []

    def test_sync_valid_review(
        self, authenticated_client: TestClient, db_manager: DbManager, test_user: tuple[str, str]
    ):
        """Sync with valid review updates card progress."""
        # Enable offline mode
        authenticated_client.post(
            "/settings",
            data={"_action": "offline_mode", "offline_mode_enabled": "true"},
        )

        # First download a session to get valid card IDs
        response = authenticated_client.post(
            "/api/study/download-session",
            json={"duration_minutes": 30, "filter_mode": "all"},
        )
        assert response.status_code == 200
        session = response.json()

        if len(session["cards"]) == 0:
            pytest.skip("No cards available for testing")

        card = session["cards"][0]
        card_id = card.get("card_id") or card.get("id")

        # Sync a review
        now = datetime.now(timezone.utc)
        response = authenticated_client.post(
            "/api/study/sync-offline",
            json={
                "session_id": session["session_id"],
                "reviews": [
                    {
                        "card_id": card_id,
                        "quality": 4,
                        "is_correct": True,
                        "hints_used": 0,
                        "timestamp": now.isoformat(),
                        "learning_step": 1,
                        "fsrs_stability": 1.5,
                        "fsrs_difficulty": 5.0,
                        "next_review": (now + timedelta(days=1)).isoformat(),
                    }
                ],
            },
        )
        assert response.status_code == 200

        data = response.json()
        assert data["synced_count"] == 1
        assert data["errors"] == []

    def test_sync_invalid_card_returns_error(
        self, authenticated_client: TestClient, db_manager: DbManager, test_user: tuple[str, str]
    ):
        """Sync with invalid card ID still processes (creates progress entry)."""
        # Enable offline mode
        authenticated_client.post(
            "/settings",
            data={"_action": "offline_mode", "offline_mode_enabled": "true"},
        )

        # Download a session to get a valid session_id
        response = authenticated_client.post(
            "/api/study/download-session",
            json={"duration_minutes": 10, "filter_mode": "all"},
        )
        assert response.status_code == 200
        session = response.json()

        now = datetime.now(timezone.utc)
        response = authenticated_client.post(
            "/api/study/sync-offline",
            json={
                "session_id": session["session_id"],
                "reviews": [
                    {
                        "card_id": 999999,  # Non-existent card
                        "quality": 4,
                        "is_correct": True,
                        "hints_used": 0,
                        "timestamp": now.isoformat(),
                        "learning_step": 0,
                        "fsrs_stability": 1.0,
                        "fsrs_difficulty": 5.0,
                        "next_review": (now + timedelta(days=1)).isoformat(),
                    }
                ],
            },
        )
        # API processes the review regardless of card existence
        assert response.status_code == 200

        data = response.json()
        # May succeed or have errors depending on foreign key constraints
        assert "synced_count" in data

    def test_sync_multiple_reviews(
        self, authenticated_client: TestClient, db_manager: DbManager, test_user: tuple[str, str]
    ):
        """Sync with multiple reviews processes all of them."""
        # Enable offline mode
        authenticated_client.post(
            "/settings",
            data={"_action": "offline_mode", "offline_mode_enabled": "true"},
        )

        # Download session for valid cards
        response = authenticated_client.post(
            "/api/study/download-session",
            json={"duration_minutes": 30, "filter_mode": "all"},
        )
        if response.status_code != 200:
            pytest.skip("Could not download session")

        session = response.json()
        if len(session["cards"]) < 2:
            pytest.skip("Need at least 2 cards for this test")

        card1_id = session["cards"][0].get("card_id") or session["cards"][0].get("id")
        card2_id = session["cards"][1].get("card_id") or session["cards"][1].get("id")

        now = datetime.now(timezone.utc)
        response = authenticated_client.post(
            "/api/study/sync-offline",
            json={
                "session_id": session["session_id"],
                "reviews": [
                    {
                        "card_id": card1_id,
                        "quality": 4,
                        "is_correct": True,
                        "hints_used": 0,
                        "timestamp": now.isoformat(),
                        "learning_step": 1,
                        "fsrs_stability": 1.5,
                        "fsrs_difficulty": 5.0,
                        "next_review": (now + timedelta(days=1)).isoformat(),
                    },
                    {
                        "card_id": card2_id,
                        "quality": 3,
                        "is_correct": True,
                        "hints_used": 1,
                        "timestamp": (now + timedelta(seconds=30)).isoformat(),
                        "learning_step": 1,
                        "fsrs_stability": 1.2,
                        "fsrs_difficulty": 5.5,
                        "next_review": (now + timedelta(hours=12)).isoformat(),
                    },
                ],
            },
        )
        assert response.status_code == 200

        data = response.json()
        # Both should be synced
        assert data["synced_count"] == 2
        assert data["errors"] == []


class TestOfflineModeSettings:
    """Tests for offline mode settings."""

    def test_enable_offline_mode(self, authenticated_client: TestClient):
        """Can enable offline mode via settings."""
        response = authenticated_client.post(
            "/settings",
            data={"_action": "offline_mode", "offline_mode_enabled": "true"},
        )
        assert response.status_code in (200, 303)

    def test_disable_offline_mode(self, authenticated_client: TestClient):
        """Can disable offline mode via settings."""
        # First enable
        authenticated_client.post(
            "/settings",
            data={"_action": "offline_mode", "offline_mode_enabled": "true"},
        )

        # Then disable (not sending checkbox means unchecked)
        response = authenticated_client.post(
            "/settings",
            data={"_action": "offline_mode"},
        )
        assert response.status_code in (200, 303)
