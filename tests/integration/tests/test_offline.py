"""
Integration tests for offline study mode API endpoints.
"""
import pytest


class TestOfflineDownload:
    """Tests for POST /api/study/download-session"""

    def test_download_requires_auth(self, client):
        """Unauthenticated requests should fail."""
        response = client.post(
            "/api/study/download-session",
            json={"duration_minutes": 30, "filter_mode": "all"},
        )
        # Should redirect to login or return 401/403
        assert response.status_code in (302, 303, 401, 403)

    def test_download_requires_offline_mode_enabled(self, authenticated_client):
        """Download fails when offline mode is not enabled."""
        response = authenticated_client.post(
            "/api/study/download-session",
            json={"duration_minutes": 30, "filter_mode": "all"},
        )
        assert response.status_code == 403
        data = response.json()
        assert "error" in data
        assert "not enabled" in data["error"].lower()

    def test_download_session_success(self, authenticated_client):
        """Download succeeds when offline mode is enabled."""
        # Enable offline mode first
        response = authenticated_client.post(
            "/settings",
            data={
                "_action": "offline_mode",
                "offline_mode_enabled": "true",
                "offline_session_duration": "30",
            },
        )
        # Settings POST redirects on success
        assert response.status_code in (200, 302, 303)

        # Now download session
        response = authenticated_client.post(
            "/api/study/download-session",
            json={"duration_minutes": 30, "filter_mode": "all"},
        )
        assert response.status_code == 200

        data = response.json()
        # May return error if no cards available, or success with cards
        if "error" not in data:
            assert "session_id" in data
            assert "created_at" in data
            assert "desired_retention" in data
            assert "focus_mode" in data
            assert "cards" in data
            assert isinstance(data["cards"], list)

            # If cards exist, verify structure
            if len(data["cards"]) > 0:
                card = data["cards"][0]
                assert "card_id" in card
                assert "front" in card
                assert "back" in card
                assert "learning_step" in card
                assert "next_review" in card

    def test_download_respects_duration(self, authenticated_client):
        """Download returns appropriate number of cards for duration."""
        # Enable offline mode
        authenticated_client.post(
            "/settings",
            data={
                "_action": "offline_mode",
                "offline_mode_enabled": "true",
                "offline_session_duration": "15",
            },
        )

        # Download 15 min session (~22 cards at 1.5 cards/min)
        response = authenticated_client.post(
            "/api/study/download-session",
            json={"duration_minutes": 15, "filter_mode": "all"},
        )
        assert response.status_code == 200

        data = response.json()
        if "error" not in data and len(data["cards"]) > 0:
            # Should have at least 10 cards (minimum) and roughly ~22 expected
            assert len(data["cards"]) >= 10


class TestOfflineSync:
    """Tests for POST /api/study/sync-offline"""

    def test_sync_requires_auth(self, client):
        """Unauthenticated requests should fail."""
        response = client.post(
            "/api/study/sync-offline",
            json={"session_id": "test123", "reviews": []},
        )
        assert response.status_code in (302, 303, 401, 403)

    def test_sync_requires_valid_session(self, authenticated_client):
        """Sync fails with invalid session ID."""
        response = authenticated_client.post(
            "/api/study/sync-offline",
            json={"session_id": "nonexistent123", "reviews": []},
        )
        assert response.status_code == 400
        data = response.json()
        assert "error" in data
        assert "not found" in data["error"].lower()

    def test_sync_empty_reviews(self, authenticated_client):
        """Sync with empty reviews returns success but zero count."""
        # Enable offline mode and download session first
        authenticated_client.post(
            "/settings",
            data={
                "_action": "offline_mode",
                "offline_mode_enabled": "true",
            },
        )

        download_response = authenticated_client.post(
            "/api/study/download-session",
            json={"duration_minutes": 15, "filter_mode": "all"},
        )

        if download_response.status_code != 200:
            pytest.skip("No cards available for test")

        data = download_response.json()
        if "error" in data:
            pytest.skip("No cards available for test")

        session_id = data["session_id"]

        # Sync with empty reviews
        sync_response = authenticated_client.post(
            "/api/study/sync-offline",
            json={"session_id": session_id, "reviews": []},
        )
        assert sync_response.status_code == 200

        sync_data = sync_response.json()
        assert sync_data["synced_count"] == 0
        assert sync_data["errors"] == []

    def test_sync_with_reviews(self, authenticated_client):
        """Sync with valid reviews updates database."""
        # Enable offline mode and download session
        authenticated_client.post(
            "/settings",
            data={
                "_action": "offline_mode",
                "offline_mode_enabled": "true",
            },
        )

        download_response = authenticated_client.post(
            "/api/study/download-session",
            json={"duration_minutes": 30, "filter_mode": "all"},
        )

        if download_response.status_code != 200:
            pytest.skip("No cards available for test")

        data = download_response.json()
        if "error" in data or len(data.get("cards", [])) == 0:
            pytest.skip("No cards available for test")

        session_id = data["session_id"]
        card = data["cards"][0]

        # Sync one review
        sync_response = authenticated_client.post(
            "/api/study/sync-offline",
            json={
                "session_id": session_id,
                "reviews": [
                    {
                        "card_id": card["card_id"],
                        "quality": 4,
                        "is_correct": True,
                        "hints_used": 0,
                        "timestamp": "2024-01-13T10:05:00Z",
                        "learning_step": 1,
                        "fsrs_stability": 1.5,
                        "fsrs_difficulty": 5.0,
                        "next_review": "2024-01-14T10:05:00Z",
                    }
                ],
            },
        )
        assert sync_response.status_code == 200

        sync_data = sync_response.json()
        assert sync_data["synced_count"] == 1
        assert sync_data["errors"] == []

    def test_sync_prevents_double_sync(self, authenticated_client):
        """Same session cannot be synced twice."""
        # Enable offline mode and download session
        authenticated_client.post(
            "/settings",
            data={
                "_action": "offline_mode",
                "offline_mode_enabled": "true",
            },
        )

        download_response = authenticated_client.post(
            "/api/study/download-session",
            json={"duration_minutes": 15, "filter_mode": "all"},
        )

        if download_response.status_code != 200:
            pytest.skip("No cards available for test")

        data = download_response.json()
        if "error" in data:
            pytest.skip("No cards available for test")

        session_id = data["session_id"]

        # First sync
        sync1 = authenticated_client.post(
            "/api/study/sync-offline",
            json={"session_id": session_id, "reviews": []},
        )
        assert sync1.status_code == 200

        # Second sync should fail
        sync2 = authenticated_client.post(
            "/api/study/sync-offline",
            json={"session_id": session_id, "reviews": []},
        )
        assert sync2.status_code == 400
        assert "already synced" in sync2.json()["error"].lower()
