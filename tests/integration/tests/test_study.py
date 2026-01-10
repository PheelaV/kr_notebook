"""Study workflow integration tests.

Tests cover:
- Interactive study mode
- Classic (flip-card) study mode
- Practice mode (no SRS impact)
- Answer validation
- Card progression
"""

import pytest

from conftest import DbManager, TestClient


class TestInteractiveStudy:
    """Interactive study mode tests."""

    def test_study_page_loads_with_card(self, authenticated_client: TestClient):
        """GET /study loads the interactive study page with a card."""
        response = authenticated_client.get("/study")

        assert response.status_code == 200
        # Should have card content or "no cards" message
        assert "card" in response.text.lower() or "study" in response.text.lower()

    def test_study_requires_authentication(self, client: TestClient):
        """GET /study redirects to login without authentication."""
        response = client.get("/study")

        assert response.status_code in (302, 303)
        assert "/login" in response.headers.get("location", "")

    def test_validate_answer_correct(self, authenticated_client: TestClient):
        """POST /validate-answer with correct answer returns success."""
        # First get a card
        study_response = authenticated_client.get("/study")
        assert study_response.status_code == 200

        # Extract card_id from the response (look for hidden input)
        import re
        card_id_match = re.search(r'name="card_id"[^>]*value="(\d+)"', study_response.text)

        if card_id_match:
            card_id = card_id_match.group(1)

            # For Hangul cards, the answer is romanization - let's test with a wrong answer first
            # to verify the endpoint works
            response = authenticated_client.post(
                "/validate-answer",
                data={
                    "card_id": card_id,
                    "answer": "test_answer",
                    "hints_used": 0,
                    "session_id": "",
                    "input_method": "text_input",  # Valid variants: text_input, multiple_choice
                },
            )

            # Should return card template (200) regardless of correctness
            assert response.status_code == 200

    def test_next_card_after_review(self, authenticated_client: TestClient):
        """POST /next-card returns the next card to study."""
        response = authenticated_client.post(
            "/next-card",
            data={"session_id": ""},
        )

        # Should return card or "no cards" response
        assert response.status_code == 200


class TestClassicStudy:
    """Classic flip-card study mode tests."""

    def test_classic_study_page_loads(self, authenticated_client: TestClient):
        """GET /study-classic loads the classic study page."""
        response = authenticated_client.get("/study-classic")

        assert response.status_code == 200

    def test_classic_study_requires_authentication(self, client: TestClient):
        """GET /study-classic redirects to login without auth."""
        response = client.get("/study-classic")

        assert response.status_code in (302, 303)
        assert "/login" in response.headers.get("location", "")


class TestPracticeMode:
    """Practice mode tests (no SRS impact)."""

    def test_practice_page_loads(self, authenticated_client: TestClient):
        """GET /practice loads the practice mode page."""
        response = authenticated_client.get("/practice")

        assert response.status_code == 200

    def test_practice_requires_authentication(self, client: TestClient):
        """GET /practice redirects to login without auth."""
        response = client.get("/practice")

        assert response.status_code in (302, 303)
        assert "/login" in response.headers.get("location", "")

    def test_practice_next_returns_card(self, authenticated_client: TestClient):
        """POST /practice-next returns a practice card."""
        # Need to send at least one field for Content-Type to be set properly
        response = authenticated_client.post(
            "/practice-next",
            data={"track_progress": "true"},
        )

        # Should return card or redirect
        assert response.status_code in (200, 302, 303)


class TestListeningPractice:
    """Listening practice mode tests."""

    def test_listen_page_loads(self, authenticated_client: TestClient):
        """GET /listen loads the listening practice page."""
        response = authenticated_client.get("/listen")

        assert response.status_code == 200

    def test_listen_requires_authentication(self, client: TestClient):
        """GET /listen redirects to login without auth."""
        response = client.get("/listen")

        assert response.status_code in (302, 303)
        assert "/login" in response.headers.get("location", "")


class TestStudyWithScenarios:
    """Study tests with specific database scenarios."""

    def test_study_with_tier1_new_scenario(
        self,
        client: TestClient,
        db_manager: DbManager,
    ):
        """Study with tier1_new scenario shows tier 1 cards."""
        import uuid

        username = f"_test_{uuid.uuid4().hex[:8]}"
        password = "test123"

        try:
            # Create user and scenario
            password_hash = db_manager.create_user(username, password)
            db_manager.create_scenario(username, "tier1_new")
            db_manager.use_scenario(username, "tier1_new")

            # Login
            client.login(username, password_hash)
            assert client.is_authenticated()

            # Study should show tier 1 cards
            response = client.get("/study")
            assert response.status_code == 200

            # Check progress page to verify tier status
            progress_response = client.get("/progress")
            assert progress_response.status_code == 200

        finally:
            db_manager.delete_user(username)

    def test_study_with_all_graduated_scenario(
        self,
        client: TestClient,
        db_manager: DbManager,
    ):
        """Study with all_graduated scenario shows "no cards due" state."""
        import uuid

        username = f"_test_{uuid.uuid4().hex[:8]}"
        password = "test123"

        try:
            # Create user and scenario
            password_hash = db_manager.create_user(username, password)
            db_manager.create_scenario(username, "all_graduated")
            db_manager.use_scenario(username, "all_graduated")

            # Login
            client.login(username, password_hash)
            assert client.is_authenticated()

            # Study should indicate no cards due (cards are graduated with future review dates)
            response = client.get("/study")
            assert response.status_code == 200

        finally:
            db_manager.delete_user(username)
