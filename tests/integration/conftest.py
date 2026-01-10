"""Pytest configuration and fixtures for integration tests.

This module provides fixtures for:
- Ephemeral test environment (isolated data directory)
- Server management (automatic spawning/teardown)
- User creation and cleanup via db-manager CLI
- HTTP client with session handling
- Database inspection utilities
"""

import hashlib
import os
import shutil
import signal
import subprocess
import time
from pathlib import Path
from typing import Generator

import httpx
import pytest

# Find project root (contains Cargo.toml)
PROJECT_ROOT = Path(__file__).parent.parent.parent
PY_SCRIPTS_DIR = PROJECT_ROOT / "py_scripts"

# Isolated test data directory (ephemeral - created fresh for each test session)
TEST_DATA_DIR = PROJECT_ROOT / "data" / "test" / "integration"
TEST_PORT = 3100


def compute_password_hash(password: str, username: str) -> str:
    """Compute client-side password hash (SHA-256 of password:username).

    This matches the authentication flow where client hashes password before sending.
    """
    combined = f"{password}:{username}"
    return hashlib.sha256(combined.encode()).hexdigest()


class DbManager:
    """Wrapper for db-manager CLI commands."""

    __test__ = False  # Prevent pytest from collecting this as a test class

    def __init__(self, project_root: Path, data_dir: Path):
        self.project_root = project_root
        self.py_scripts_dir = project_root / "py_scripts"
        self.data_dir = data_dir

    def _run(self, *args: str, check: bool = True) -> subprocess.CompletedProcess:
        """Run db-manager with given arguments."""
        cmd = ["uv", "run", "db-manager", *args, "--data-dir", str(self.data_dir)]
        return subprocess.run(
            cmd,
            cwd=self.py_scripts_dir,
            capture_output=True,
            text=True,
            check=check,
        )

    def create_user(self, username: str, password: str = "test123") -> str:
        """Create a test user and return the password hash for login."""
        # Use create-test-user which supports --data-dir
        self._run("create-test-user", username, "--password", password)
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

    def get_group_count(self) -> int:
        """Get the total number of groups."""
        result = self._run("get-group-count", check=False)
        try:
            return int(result.stdout.strip())
        except ValueError:
            return 0

    def get_guest_count(self) -> int:
        """Get the total number of guest users."""
        result = self._run("get-guest-count", check=False)
        try:
            return int(result.stdout.strip())
        except ValueError:
            return 0

    def group_exists(self, group_id: str) -> bool:
        """Check if a group exists."""
        result = self._run("list-groups", check=False)
        return group_id in result.stdout


class TestClient:
    """HTTP client wrapper with session/cookie management."""

    __test__ = False  # Prevent pytest from collecting this as a test class

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
        """Extract and store cookies from response, updating client cookies."""
        for cookie in response.cookies.jar:
            if cookie.name == "kr_session":
                self.session_cookie = cookie.value
                self.client.cookies.set("kr_session", cookie.value)
            elif cookie.name == "kr_username":
                self.username_cookie = cookie.value
                self.client.cookies.set("kr_username", cookie.value)

    def _sync_cookies(self) -> None:
        """Ensure client cookies are in sync with our tracked cookies."""
        if self.session_cookie:
            self.client.cookies.set("kr_session", self.session_cookie)
        else:
            self.client.cookies.delete("kr_session")
        if self.username_cookie:
            self.client.cookies.set("kr_username", self.username_cookie)
        else:
            self.client.cookies.delete("kr_username")

    def get(self, path: str, **kwargs) -> httpx.Response:
        """Make a GET request."""
        self._sync_cookies()
        response = self.client.get(path, **kwargs)
        self._update_cookies(response)
        return response

    def post(self, path: str, **kwargs) -> httpx.Response:
        """Make a POST request."""
        self._sync_cookies()
        response = self.client.post(path, **kwargs)
        self._update_cookies(response)
        return response

    def delete(self, path: str, **kwargs) -> httpx.Response:
        """Make a DELETE request."""
        self._sync_cookies()
        response = self.client.delete(path, **kwargs)
        self._update_cookies(response)
        return response

    def request(self, method: str, path: str, **kwargs) -> httpx.Response:
        """Make a request with arbitrary HTTP method."""
        self._sync_cookies()
        response = self.client.request(method, path, **kwargs)
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
        self.client.cookies.clear()
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


def wait_for_server(url: str, timeout: float = 60.0) -> bool:
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


@pytest.fixture(scope="session")
def project_root() -> Path:
    """Return the project root path."""
    return PROJECT_ROOT


@pytest.fixture(scope="session")
def test_data_dir() -> Path:
    """Return the isolated test data directory."""
    return TEST_DATA_DIR


@pytest.fixture(scope="session")
def test_server(project_root: Path, test_data_dir: Path) -> Generator[str, None, None]:
    """Spawn an isolated test server with ephemeral data directory.

    This fixture:
    1. Creates a fresh test data directory
    2. Initializes the test environment via db-manager
    3. Spawns a server with the isolated DATA_DIR
    4. Yields the server URL
    5. Terminates the server and cleans up (unless PRESERVE_TEST_ENV is set)
    """
    # Clean up any existing test directory
    if test_data_dir.exists():
        shutil.rmtree(test_data_dir)

    # Initialize test environment via db-manager
    init_result = subprocess.run(
        ["uv", "run", "db-manager", "init-test-env", "integration",
         "--data-dir", str(test_data_dir)],
        cwd=project_root / "py_scripts",
        capture_output=True,
        text=True,
    )
    if init_result.returncode != 0:
        pytest.fail(f"Failed to initialize test environment: {init_result.stderr}")

    # Spawn server with isolated data directory
    env = os.environ.copy()
    env["DATA_DIR"] = str(test_data_dir)
    env["PORT"] = str(TEST_PORT)

    process = subprocess.Popen(
        ["cargo", "run", "--quiet"],
        cwd=project_root,
        env=env,
        stdout=subprocess.DEVNULL,
        stderr=subprocess.DEVNULL,
        preexec_fn=os.setsid,  # Create new process group for clean termination
    )

    url = f"http://localhost:{TEST_PORT}"

    # Wait for server to be ready
    if not wait_for_server(url, timeout=60.0):
        process.terminate()
        process.wait(timeout=5)
        pytest.fail(f"Test server failed to start at {url}")

    yield url

    # Terminate server
    try:
        os.killpg(os.getpgid(process.pid), signal.SIGTERM)
        process.wait(timeout=5)
    except (ProcessLookupError, OSError):
        pass  # Process already terminated

    # Cleanup test data directory unless PRESERVE_TEST_ENV is set
    if not os.environ.get("PRESERVE_TEST_ENV"):
        shutil.rmtree(test_data_dir, ignore_errors=True)


@pytest.fixture(scope="session")
def server_url(test_server: str) -> str:
    """Return the test server URL."""
    return test_server


@pytest.fixture(scope="session")
def db_manager(project_root: Path, test_data_dir: Path, test_server: str) -> DbManager:
    """Return a db-manager wrapper for the isolated test environment.

    Depends on test_server to ensure environment is initialized.
    """
    return DbManager(project_root, data_dir=test_data_dir)


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
