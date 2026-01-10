"""Authentication integration tests.

Tests cover:
- User registration flow
- Login with valid/invalid credentials
- Session cookie handling
- Logout and session cleanup
- Protected route access control
- Guest account creation
"""

import uuid

import pytest

from conftest import DbManager, TestClient, compute_password_hash


class TestLogin:
    """Login flow tests."""

    def test_login_page_accessible(self, client: TestClient):
        """GET /login returns the login page."""
        response = client.get("/login")
        assert response.status_code == 200
        assert "login" in response.text.lower() or "password" in response.text.lower()

    def test_login_with_valid_credentials(
        self, client: TestClient, test_user: tuple[str, str]
    ):
        """POST /login with valid credentials sets session cookie and redirects."""
        username, password_hash = test_user

        response = client.login(username, password_hash)

        # Should redirect to home
        assert response.status_code in (302, 303)
        assert response.headers.get("location") == "/"

        # Session cookie should be set
        assert client.session_cookie is not None
        assert client.username_cookie == username

    def test_login_with_invalid_password(
        self, client: TestClient, test_user: tuple[str, str]
    ):
        """POST /login with wrong password shows error."""
        username, _ = test_user
        wrong_hash = compute_password_hash("wrongpassword", username)

        response = client.login(username, wrong_hash)

        # Should return login page with error (not redirect)
        assert response.status_code == 200
        assert "invalid" in response.text.lower() or "error" in response.text.lower()
        assert client.session_cookie is None

    def test_login_with_nonexistent_user(self, client: TestClient):
        """POST /login with nonexistent user shows error."""
        password_hash = compute_password_hash("test", "nonexistent_user_xyz")

        response = client.login("nonexistent_user_xyz", password_hash)

        assert response.status_code == 200
        assert "invalid" in response.text.lower()
        assert client.session_cookie is None

    def test_login_with_empty_fields(self, client: TestClient):
        """POST /login with empty fields shows error."""
        response = client.post("/login", data={"username": "", "password_hash": ""})

        assert response.status_code == 200
        assert client.session_cookie is None


class TestLogout:
    """Logout flow tests."""

    def test_logout_clears_session(
        self, authenticated_client: TestClient, test_user: tuple[str, str]
    ):
        """POST /logout clears session cookie and redirects to login."""
        assert authenticated_client.is_authenticated()

        response = authenticated_client.logout()

        assert response.status_code in (302, 303)
        assert "/login" in response.headers.get("location", "")
        assert not authenticated_client.is_authenticated()

    def test_logout_without_session(self, client: TestClient):
        """POST /logout without session still redirects to login."""
        response = client.logout()

        assert response.status_code in (302, 303)
        assert "/login" in response.headers.get("location", "")


class TestProtectedRoutes:
    """Protected route access control tests."""

    @pytest.mark.parametrize(
        "path",
        [
            "/",
            "/study",
            "/progress",
            "/settings",
            "/library",
        ],
    )
    def test_protected_routes_redirect_to_login(self, client: TestClient, path: str):
        """Protected routes redirect unauthenticated users to /login."""
        response = client.get(path)

        assert response.status_code in (302, 303)
        assert "/login" in response.headers.get("location", "")

    @pytest.mark.parametrize(
        "path",
        [
            "/",
            "/study",
            "/progress",
            "/settings",
            "/library",
        ],
    )
    def test_protected_routes_accessible_when_authenticated(
        self, authenticated_client: TestClient, path: str
    ):
        """Protected routes are accessible with valid session."""
        response = authenticated_client.get(path)

        # Should return 200 or redirect within the app (not to login)
        if response.status_code in (302, 303):
            location = response.headers.get("location", "")
            assert "/login" not in location
        else:
            assert response.status_code == 200


class TestPublicRoutes:
    """Public route access tests."""

    @pytest.mark.parametrize(
        "path",
        [
            "/login",
            "/register",
            "/guest",
            "/reference",
            "/guide",
        ],
    )
    def test_public_routes_accessible_without_auth(self, client: TestClient, path: str):
        """Public routes are accessible without authentication."""
        response = client.get(path)

        # Should return 200 (not redirect to login)
        assert response.status_code == 200


class TestRegistration:
    """User registration tests."""

    def test_register_page_accessible(self, client: TestClient):
        """GET /register returns the registration page."""
        response = client.get("/register")
        assert response.status_code == 200
        assert "register" in response.text.lower()

    def test_register_creates_user_and_logs_in(
        self, client: TestClient, db_manager: DbManager
    ):
        """POST /register creates user, sets session, and redirects home."""
        username = f"_test_{uuid.uuid4().hex[:8]}"
        password = "testpass123"
        password_hash = compute_password_hash(password, username)

        try:
            response = client.post(
                "/register",
                data={"username": username, "password_hash": password_hash},
            )

            # Should redirect to home after successful registration
            assert response.status_code in (302, 303)
            assert response.headers.get("location") == "/"

            # Should be logged in
            assert client.is_authenticated()
            assert client.username_cookie == username

            # User should exist in database
            assert db_manager.user_exists(username)

        finally:
            # Cleanup
            db_manager.delete_user(username)

    def test_register_with_existing_username(
        self, client: TestClient, test_user: tuple[str, str]
    ):
        """POST /register with existing username shows error."""
        username, _ = test_user
        password_hash = compute_password_hash("newpass", username)

        response = client.post(
            "/register",
            data={"username": username, "password_hash": password_hash},
        )

        # Should return register page with error
        assert response.status_code == 200
        assert "exists" in response.text.lower() or "already" in response.text.lower()

    def test_register_with_invalid_username(self, client: TestClient):
        """POST /register with invalid username shows error."""
        # Username too short
        password_hash = compute_password_hash("test", "ab")

        response = client.post(
            "/register",
            data={"username": "ab", "password_hash": password_hash},
        )

        assert response.status_code == 200
        assert client.session_cookie is None


class TestGuestLogin:
    """Guest account tests."""

    def test_guest_page_accessible(self, client: TestClient):
        """GET /guest returns the guest login page."""
        response = client.get("/guest")
        assert response.status_code == 200

    def test_guest_login_creates_account(
        self, client: TestClient, db_manager: DbManager
    ):
        """POST /guest creates a guest account and logs in."""
        response = client.post("/guest", data={"nickname": ""})

        # Should redirect to home
        assert response.status_code in (302, 303)
        assert response.headers.get("location") == "/"

        # Should be logged in
        assert client.is_authenticated()

        # Username should start with _guest_
        username = client.username_cookie
        assert username is not None
        assert username.startswith("_guest_")

        # Cleanup
        db_manager.delete_user(username)

    def test_guest_login_with_nickname(
        self, client: TestClient, db_manager: DbManager
    ):
        """POST /guest with nickname uses sanitized nickname."""
        nickname = f"test{uuid.uuid4().hex[:4]}"

        response = client.post("/guest", data={"nickname": nickname})

        assert response.status_code in (302, 303)
        assert client.is_authenticated()

        username = client.username_cookie
        assert username is not None
        assert nickname.lower() in username.lower()

        # Cleanup
        db_manager.delete_user(username)


class TestSessionPersistence:
    """Session persistence and cookie handling tests."""

    def test_session_persists_across_requests(
        self, authenticated_client: TestClient
    ):
        """Session cookie is preserved across multiple requests."""
        initial_session = authenticated_client.session_cookie

        # Make multiple requests
        authenticated_client.get("/")
        authenticated_client.get("/study")
        authenticated_client.get("/progress")

        # Session should still be the same
        assert authenticated_client.session_cookie == initial_session
        assert authenticated_client.is_authenticated()

    def test_invalid_session_cookie_redirects_to_login(self, client: TestClient):
        """Invalid session cookie results in redirect to login."""
        # Set an invalid session cookie
        client.session_cookie = "invalid_session_id_12345"

        response = client.get("/")

        assert response.status_code in (302, 303)
        assert "/login" in response.headers.get("location", "")
