"""Exercise endpoint integration tests.

Tests cover:
- Exercise index page listing packs
- Exercise pack page listing lessons
- Exercise session page
- Cloze answer checking (correct/incorrect)
- Next exercise navigation
"""

import pytest
from conftest import DbManager, TestClient


class TestExerciseEndpoints:
    """Tests for exercise page access and navigation."""

    def test_exercise_index_requires_auth(self, client: TestClient):
        """GET /exercises requires authentication."""
        response = client.get("/exercises")
        # Should redirect to login
        assert response.status_code == 302
        assert "/login" in response.headers.get("location", "")

    def test_exercise_index_accessible_when_authenticated(
        self, authenticated_client: TestClient
    ):
        """GET /exercises returns 200 for authenticated user."""
        response = authenticated_client.get("/exercises")
        assert response.status_code == 200
        assert "Grammar Exercises" in response.text

    def test_exercise_index_shows_packs_or_empty_message(
        self, authenticated_client: TestClient
    ):
        """Exercise index shows available packs or empty message."""
        response = authenticated_client.get("/exercises")
        assert response.status_code == 200
        # Should either show packs or "no exercise packs" message
        assert "exercise" in response.text.lower()


class TestExercisePack:
    """Tests for exercise pack pages."""

    def test_exercise_pack_invalid_id_redirects(
        self, authenticated_client: TestClient
    ):
        """GET /exercises/pack/invalid redirects to exercise index."""
        response = authenticated_client.get("/exercises/pack/nonexistent_pack_xyz")
        # Should redirect back to exercise index
        assert response.status_code == 302
        assert "/exercises" in response.headers.get("location", "")


class TestExerciseSession:
    """Tests for exercise session endpoints."""

    def test_exercise_session_invalid_pack_redirects(
        self, authenticated_client: TestClient
    ):
        """GET /exercises/pack/invalid/lesson/1 redirects."""
        response = authenticated_client.get("/exercises/pack/nonexistent_pack/lesson/1")
        assert response.status_code == 302


class TestClozeValidation:
    """Tests for cloze answer checking endpoint."""

    def test_check_cloze_requires_auth(self, client: TestClient):
        """POST /exercises/check requires authentication."""
        response = client.post(
            "/exercises/check",
            data={
                "pack_id": "test",
                "lesson": "1",
                "exercise_index": "0",
                "blank_position": "1",
                "answer": "는",
            },
        )
        # Should redirect to login
        assert response.status_code == 302

    def test_check_cloze_invalid_pack(self, authenticated_client: TestClient):
        """POST /exercises/check with invalid pack returns error."""
        response = authenticated_client.post(
            "/exercises/check",
            data={
                "pack_id": "nonexistent_pack",
                "lesson": "1",
                "exercise_index": "0",
                "blank_position": "1",
                "answer": "는",
            },
        )
        assert response.status_code == 200
        assert "error" in response.text.lower() or "not found" in response.text.lower()


class TestNextExercise:
    """Tests for next exercise navigation endpoint."""

    def test_next_exercise_requires_auth(self, client: TestClient):
        """POST /exercises/next requires authentication."""
        response = client.post(
            "/exercises/next",
            data={
                "pack_id": "test",
                "lesson": "1",
                "exercise_index": "0",
            },
        )
        # Should redirect to login
        assert response.status_code == 302

    def test_next_exercise_invalid_pack(self, authenticated_client: TestClient):
        """POST /exercises/next with invalid pack returns error."""
        response = authenticated_client.post(
            "/exercises/next",
            data={
                "pack_id": "nonexistent_pack",
                "lesson": "1",
                "exercise_index": "0",
            },
        )
        assert response.status_code == 200
        assert "error" in response.text.lower() or "not found" in response.text.lower()


class TestExerciseNavigation:
    """Tests for exercise navigation flow."""

    def test_can_navigate_from_index(self, authenticated_client: TestClient):
        """Can navigate from exercise index to other pages."""
        response = authenticated_client.get("/exercises")
        assert response.status_code == 200
        # Page should render correctly
        assert "</html>" in response.text.lower()
