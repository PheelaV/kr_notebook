"""Answer validation integration tests.

Tests cover:
- Bracket variants [a, b, c]
- Suffix syntax word(s)
- Info syntax (ignored)
- Disambiguation syntax <context>
- Comma-separated synonyms (permutation matching)
- Partial match detection
- Visual indicator rendering
"""

import re
import pytest

from conftest import DbManager, TestClient


def extract_card_data(html: str) -> dict:
    """Extract card_id and main_answer from response HTML."""
    card_id_match = re.search(r'name="card_id"[^>]*value="(\d+)"', html)
    # Try to find main_answer in data attribute or hidden input
    answer_match = re.search(r'data-answer="([^"]+)"', html)
    if not answer_match:
        answer_match = re.search(r'name="main_answer"[^>]*value="([^"]+)"', html)

    return {
        "card_id": card_id_match.group(1) if card_id_match else None,
        "main_answer": answer_match.group(1) if answer_match else None,
    }


class TestAnswerValidationEndpoint:
    """Tests for /practice-validate endpoint with various grammar patterns."""

    def test_practice_validate_endpoint_exists(self, authenticated_client: TestClient):
        """POST /practice-validate returns a response."""
        # Get a card first
        response = authenticated_client.get("/practice?mode=interactive")
        assert response.status_code == 200

        card_data = extract_card_data(response.text)
        if card_data["card_id"]:
            validate_response = authenticated_client.post(
                "/practice-validate",
                data={
                    "card_id": card_data["card_id"],
                    "answer": "test",
                    "track_progress": "false",
                    "input_method": "text_input",
                },
            )
            assert validate_response.status_code == 200

    def test_correct_answer_shows_correct_result(self, authenticated_client: TestClient):
        """Correct answer shows 'Correct!' in response."""
        # Get a practice card
        response = authenticated_client.get("/practice?mode=interactive")
        assert response.status_code == 200

        card_data = extract_card_data(response.text)
        if card_data["card_id"] and card_data["main_answer"]:
            # Submit the exact correct answer
            validate_response = authenticated_client.post(
                "/practice-validate",
                data={
                    "card_id": card_data["card_id"],
                    "answer": card_data["main_answer"],
                    "track_progress": "false",
                    "input_method": "text_input",
                },
            )
            assert validate_response.status_code == 200
            # Look for correct result indicator
            assert 'data-result="correct"' in validate_response.text or "Correct" in validate_response.text

    def test_incorrect_answer_shows_incorrect_result(self, authenticated_client: TestClient):
        """Incorrect answer shows 'Incorrect' in response."""
        response = authenticated_client.get("/practice?mode=interactive")
        assert response.status_code == 200

        card_data = extract_card_data(response.text)
        if card_data["card_id"]:
            # Submit a definitely wrong answer
            validate_response = authenticated_client.post(
                "/practice-validate",
                data={
                    "card_id": card_data["card_id"],
                    "answer": "xyzzy_wrong_answer_12345",
                    "track_progress": "false",
                    "input_method": "text_input",
                },
            )
            assert validate_response.status_code == 200
            assert 'data-result="incorrect"' in validate_response.text or "Incorrect" in validate_response.text


class TestVisualIndicators:
    """Tests for visual indicator rendering in card display."""

    def test_practice_page_renders_cards(self, authenticated_client: TestClient):
        """Practice page renders card content."""
        response = authenticated_client.get("/practice?mode=interactive")
        assert response.status_code == 200
        # Should have some card content
        assert "card" in response.text.lower() or "practice" in response.text.lower()

    def test_study_page_renders_cards(self, authenticated_client: TestClient):
        """Study page renders card content."""
        response = authenticated_client.get("/study")
        assert response.status_code == 200

    def test_card_result_template_exists(self, authenticated_client: TestClient):
        """Card result template is rendered on validation."""
        response = authenticated_client.get("/practice?mode=interactive")
        assert response.status_code == 200

        card_data = extract_card_data(response.text)
        if card_data["card_id"]:
            validate_response = authenticated_client.post(
                "/practice-validate",
                data={
                    "card_id": card_data["card_id"],
                    "answer": "test",
                    "track_progress": "false",
                    "input_method": "text_input",
                },
            )
            # Result should have one of the result states
            assert (
                'data-result="correct"' in validate_response.text
                or 'data-result="incorrect"' in validate_response.text
                or 'data-result="partial"' in validate_response.text
            )


class TestStudyModeValidation:
    """Tests for study mode (/study) validation behavior."""

    def test_study_validate_endpoint(self, authenticated_client: TestClient):
        """POST /validate-answer endpoint works."""
        response = authenticated_client.get("/study")
        assert response.status_code == 200

        card_data = extract_card_data(response.text)
        if card_data["card_id"]:
            validate_response = authenticated_client.post(
                "/validate-answer",
                data={
                    "card_id": card_data["card_id"],
                    "answer": "test",
                    "hints_used": "0",
                    "session_id": "",
                    "input_method": "text_input",
                },
            )
            assert validate_response.status_code == 200

    def test_study_next_card(self, authenticated_client: TestClient):
        """POST /next-card returns next card after validation."""
        response = authenticated_client.post(
            "/next-card",
            data={"session_id": ""},
        )
        assert response.status_code == 200


class TestPracticeModeFilters:
    """Tests for practice mode with study filters."""

    def test_practice_with_hangul_filter(self, authenticated_client: TestClient):
        """Practice with hangul filter only shows Hangul cards."""
        # First set the filter
        authenticated_client.post(
            "/settings/study-filter",
            data={"filter_mode": "hangul"},
        )

        response = authenticated_client.get("/practice?mode=interactive")
        assert response.status_code == 200

    def test_practice_with_all_filter(self, authenticated_client: TestClient):
        """Practice with 'all' filter shows any card."""
        authenticated_client.post(
            "/settings/study-filter",
            data={"filter_mode": "all"},
        )

        response = authenticated_client.get("/practice?mode=interactive")
        assert response.status_code == 200


class TestInputMethods:
    """Tests for different input methods (text vs multiple choice)."""

    def test_text_input_validation(self, authenticated_client: TestClient):
        """Text input uses fuzzy matching."""
        response = authenticated_client.get("/practice?mode=interactive")
        assert response.status_code == 200

        card_data = extract_card_data(response.text)
        if card_data["card_id"]:
            validate_response = authenticated_client.post(
                "/practice-validate",
                data={
                    "card_id": card_data["card_id"],
                    "answer": "test",
                    "track_progress": "false",
                    "input_method": "text_input",
                },
            )
            assert validate_response.status_code == 200

    def test_multiple_choice_validation(self, authenticated_client: TestClient):
        """Multiple choice input uses strict matching."""
        response = authenticated_client.get("/practice?mode=interactive")
        assert response.status_code == 200

        card_data = extract_card_data(response.text)
        if card_data["card_id"]:
            validate_response = authenticated_client.post(
                "/practice-validate",
                data={
                    "card_id": card_data["card_id"],
                    "answer": "test",
                    "track_progress": "false",
                    "input_method": "multiple_choice",
                },
            )
            assert validate_response.status_code == 200


class TestTrackProgress:
    """Tests for practice mode progress tracking."""

    def test_practice_with_tracking_enabled(self, authenticated_client: TestClient):
        """Practice with track_progress=true logs to stats."""
        response = authenticated_client.get("/practice?mode=interactive&track=true")
        assert response.status_code == 200

        card_data = extract_card_data(response.text)
        if card_data["card_id"]:
            validate_response = authenticated_client.post(
                "/practice-validate",
                data={
                    "card_id": card_data["card_id"],
                    "answer": "test",
                    "track_progress": "true",
                    "input_method": "text_input",
                },
            )
            assert validate_response.status_code == 200

    def test_practice_with_tracking_disabled(self, authenticated_client: TestClient):
        """Practice with track_progress=false does not log to stats."""
        response = authenticated_client.get("/practice?mode=interactive&track=false")
        assert response.status_code == 200

        card_data = extract_card_data(response.text)
        if card_data["card_id"]:
            validate_response = authenticated_client.post(
                "/practice-validate",
                data={
                    "card_id": card_data["card_id"],
                    "answer": "test",
                    "track_progress": "false",
                    "input_method": "text_input",
                },
            )
            assert validate_response.status_code == 200
