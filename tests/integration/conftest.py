"""Pytest configuration and fixtures for integration tests.

This module provides fixtures for:
- Server management (starting/stopping the test server)
- User creation and cleanup via db-manager CLI
- HTTP client with session handling
- Database inspection utilities
"""

import hashlib
import os
import subprocess
import time
from pathlib import Path
from typing import Generator

import httpx
import pytest

# Find project root (contains Cargo.toml)
PROJECT_ROOT = Path(__file__).parent.parent.parent
PY_SCRIPTS_DIR = PROJECT_ROOT / "py_scripts"
DATA_DIR = PROJECT_ROOT / "data"


def compute_password_hash(password: str, username: str) -> str:
    """Compute client-side password hash (SHA-256 of password:username).

    This matches the authentication flow where client hashes password before sending.
    """
    combined = f"{password}:{username}"
    return hashlib.sha256(combined.encode()).hexdigest()


class DbManager:
    """Wrapper for db-manager CLI commands."""

    def __init__(self, project_root: Path):
        self.project_root = project_root
        self.py_scripts_dir = project_root / "py_scripts"

    def _run(self, *args: str, check: bool = True) -> subprocess.CompletedProcess:
        """Run db-manager with given arguments."""
        cmd = ["uv", "run", "db-manager", *args]
        return subprocess.run(
            cmd,
            cwd=self.py_scripts_dir,
            capture_output=True,
            text=True,
            check=check,
        )

    def create_user(
        self, username: str, password: str = "test123", guest: bool = False
    ) -> str:
        """Create a test user and return the password hash for login."""
        args = ["create-user", username, "--password", password]
        if guest:
            args.append("--guest")
        self._run(*args)
        return compute_password_hash(password, username)

    def delete_user(self, username: str) -> None:
        """Delete a user and all their data."""
        self._run("delete-user", username, "--yes", check=False)

    def user_exists(self, username: str) -> bool:
        """Check if a user exists."""
        result = self._run("list-users", check=False)
        return username in result.stdout

    def create_scenario(self, username: str, preset: str) -> None:
        """Create a test scenario for a user."""
        self._run("create-scenario", preset, "--user", username)

    def use_scenario(self, username: str, scenario: str) -> None:
        """Switch user to a scenario."""
        self._run("use", scenario, "--user", username)


class TestClient:
    """HTTP client wrapper with session/cookie management."""

    def __init__(self, base_url: str = "http://localhost:3000"):
        self.base_url = base_url
        self.client = httpx.Client(
            base_url=base_url,
            follow_redirects=False,  # We want to inspect redirects
            timeout=30.0,
        )
        self.session_cookie: str | None = None
        self.username_cookie: str | None = None

    def close(self) -> None:
        """Close the HTTP client."""
        self.client.close()

    def _update_cookies(self, response: httpx.Response) -> None:
        """Extract and store cookies from response."""
        for cookie in response.cookies.jar:
            if cookie.name == "kr_session":
                self.session_cookie = cookie.value
            elif cookie.name == "kr_username":
                self.username_cookie = cookie.value

    def _get_cookies(self) -> dict[str, str]:
        """Get cookies dict for requests."""
        cookies = {}
        if self.session_cookie:
            cookies["kr_session"] = self.session_cookie
        if self.username_cookie:
            cookies["kr_username"] = self.username_cookie
        return cookies

    def get(self, path: str, **kwargs) -> httpx.Response:
        """Make a GET request."""
        kwargs.setdefault("cookies", self._get_cookies())
        response = self.client.get(path, **kwargs)
        self._update_cookies(response)
        return response

    def post(self, path: str, **kwargs) -> httpx.Response:
        """Make a POST request."""
        kwargs.setdefault("cookies", self._get_cookies())
        response = self.client.post(path, **kwargs)
        self._update_cookies(response)
        return response

    def login(self, username: str, password_hash: str) -> httpx.Response:
        """Login with username and pre-computed password hash."""
        return self.post(
            "/login",
            data={"username": username, "password_hash": password_hash},
        )

    def logout(self) -> httpx.Response:
        """Logout and clear session."""
        response = self.post("/logout")
        self.session_cookie = None
        self.username_cookie = None
        return response

    def is_authenticated(self) -> bool:
        """Check if client has a session cookie."""
        return self.session_cookie is not None

    def follow_redirect(self, response: httpx.Response) -> httpx.Response:
        """Follow a redirect response."""
        if response.status_code in (301, 302, 303, 307, 308):
            location = response.headers.get("location", "/")
            return self.get(location)
        return response


@pytest.fixture(scope="session")
def project_root() -> Path:
    """Return the project root path."""
    return PROJECT_ROOT


@pytest.fixture(scope="session")
def db_manager(project_root: Path) -> DbManager:
    """Return a db-manager wrapper instance."""
    return DbManager(project_root)


@pytest.fixture(scope="session")
def server_url() -> str:
    """Return the server URL.

    By default uses localhost:3000. Set KR_TEST_URL env var to override.
    """
    return os.environ.get("KR_TEST_URL", "http://localhost:3000")


@pytest.fixture
def client(server_url: str) -> Generator[TestClient, None, None]:
    """Create an HTTP client for testing."""
    test_client = TestClient(server_url)
    yield test_client
    test_client.close()


@pytest.fixture
def authenticated_client(
    client: TestClient, db_manager: DbManager, test_user: tuple[str, str]
) -> TestClient:
    """Create an authenticated HTTP client."""
    username, password_hash = test_user
    response = client.login(username, password_hash)
    # Follow redirect after login
    if response.status_code in (302, 303):
        client.follow_redirect(response)
    assert client.is_authenticated(), "Failed to authenticate"
    return client


@pytest.fixture
def test_user(db_manager: DbManager) -> Generator[tuple[str, str], None, None]:
    """Create a test user and return (username, password_hash).

    User is automatically cleaned up after the test.
    """
    import uuid

    username = f"_test_{uuid.uuid4().hex[:8]}"
    password = "test123"

    # Clean up if exists from previous failed test
    if db_manager.user_exists(username):
        db_manager.delete_user(username)

    password_hash = db_manager.create_user(username, password)

    yield (username, password_hash)

    # Cleanup
    db_manager.delete_user(username)


@pytest.fixture
def admin_user(db_manager: DbManager) -> Generator[tuple[str, str], None, None]:
    """Create an admin user for testing admin functionality.

    Note: This requires manual role assignment or using 'admin' username.
    """
    # The app treats 'admin' username as admin by default
    username = "admin"
    password = "admintest123"

    # Only create if not exists
    if not db_manager.user_exists(username):
        password_hash = db_manager.create_user(username, password)
    else:
        password_hash = compute_password_hash(password, username)

    yield (username, password_hash)

    # Don't delete admin user - it may be needed by other tests


def wait_for_server(url: str, timeout: float = 30.0) -> bool:
    """Wait for server to become available."""
    start = time.time()
    while time.time() - start < timeout:
        try:
            response = httpx.get(f"{url}/login", timeout=2.0)
            if response.status_code in (200, 302):
                return True
        except httpx.RequestError:
            pass
        time.sleep(0.5)
    return False


@pytest.fixture(scope="session", autouse=True)
def ensure_server_running(server_url: str) -> None:
    """Ensure the server is running before tests.

    This fixture doesn't start the server - it expects it to be running.
    Tests will fail fast if server is not available.
    """
    if not wait_for_server(server_url, timeout=5.0):
        pytest.skip(
            f"Server not running at {server_url}. "
            "Start it with 'cargo run' before running integration tests."
        )
