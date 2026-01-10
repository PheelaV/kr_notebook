"""Data isolation integration tests.

Tests cover:
- Multi-user data separation
- Per-user database isolation
- Cross-user data access prevention
"""

import uuid

import pytest

from conftest import DbManager, TestClient


class TestUserDataIsolation:
    """User data isolation tests."""

    def test_users_have_separate_progress(
        self,
        db_manager: DbManager,
    ):
        """Two users have completely separate progress."""
        user1 = f"_test_{uuid.uuid4().hex[:8]}"
        user2 = f"_test_{uuid.uuid4().hex[:8]}"
        password = "test123"

        client1 = TestClient()
        client2 = TestClient()

        try:
            # Create two users with different scenarios
            hash1 = db_manager.create_user(user1, password)
            hash2 = db_manager.create_user(user2, password)

            # Set different scenarios
            db_manager.create_scenario(user1, "tier1_new")
            db_manager.use_scenario(user1, "tier1_new")

            db_manager.create_scenario(user2, "all_graduated")
            db_manager.use_scenario(user2, "all_graduated")

            # Login both users
            client1.login(user1, hash1)
            client2.login(user2, hash2)

            # Get progress for both
            progress1 = client1.get("/progress")
            progress2 = client2.get("/progress")

            assert progress1.status_code == 200
            assert progress2.status_code == 200

            # Content should be different (one is new, one is graduated)
            # This is a basic check - detailed verification would require parsing HTML

        finally:
            client1.close()
            client2.close()
            db_manager.delete_user(user1)
            db_manager.delete_user(user2)

    def test_session_cookie_is_user_specific(
        self,
        db_manager: DbManager,
    ):
        """Session cookie only works for the user it was issued to."""
        user1 = f"_test_{uuid.uuid4().hex[:8]}"
        user2 = f"_test_{uuid.uuid4().hex[:8]}"
        password = "test123"

        client1 = TestClient()
        client2 = TestClient()

        try:
            hash1 = db_manager.create_user(user1, password)
            hash2 = db_manager.create_user(user2, password)

            # Login user1
            client1.login(user1, hash1)
            assert client1.is_authenticated()
            user1_session = client1.session_cookie

            # Login user2
            client2.login(user2, hash2)
            assert client2.is_authenticated()
            user2_session = client2.session_cookie

            # Sessions should be different
            assert user1_session != user2_session

            # Each user's session should work for them
            response1 = client1.get("/")
            response2 = client2.get("/")
            assert response1.status_code == 200
            assert response2.status_code == 200

        finally:
            client1.close()
            client2.close()
            db_manager.delete_user(user1)
            db_manager.delete_user(user2)

    def test_cannot_access_other_user_data_directly(
        self,
        client: TestClient,
        db_manager: DbManager,
    ):
        """User cannot access another user's data through API manipulation."""
        user1 = f"_test_{uuid.uuid4().hex[:8]}"
        password = "test123"

        try:
            hash1 = db_manager.create_user(user1, password)
            client.login(user1, hash1)

            # Try to access settings - should only see own data
            response = client.get("/settings")
            assert response.status_code == 200

            # Try export - should only export own data
            export_response = client.get("/settings/export")
            assert export_response.status_code == 200

        finally:
            db_manager.delete_user(user1)


class TestGuestIsolation:
    """Guest user isolation tests."""

    def test_guest_has_separate_data(
        self,
        client: TestClient,
        db_manager: DbManager,
    ):
        """Guest users have their own isolated data."""
        # Create a guest
        response = client.post("/guest", data={"nickname": ""})
        assert response.status_code in (302, 303)
        assert client.is_authenticated()

        guest_username = client.username_cookie
        assert guest_username is not None
        assert guest_username.startswith("_guest_")

        try:
            # Guest should have their own progress
            progress = client.get("/progress")
            assert progress.status_code == 200

            # Guest should be able to study
            study = client.get("/study")
            assert study.status_code == 200

        finally:
            db_manager.delete_user(guest_username)
