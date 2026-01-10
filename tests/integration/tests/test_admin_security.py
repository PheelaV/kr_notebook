"""Admin endpoint security tests.

Tests verify that admin-only endpoints:
1. Reject unauthenticated requests (redirect to /login)
2. Reject unauthorized requests from non-admin users (403 or redirect to /settings)
3. Do NOT perform the action (side-effect verification where applicable)

This provides exhaustive coverage of all 24 admin-only endpoints from a security
perspective, ensuring the API is sealed against unauthorized access.
"""

import pytest

from conftest import DbManager, TestClient


# All admin-only endpoints with their HTTP methods and sample data
ADMIN_ENDPOINTS = [
    # Guest Management
    ("POST", "/settings/cleanup-guests", {}),
    ("POST", "/settings/delete-all-guests", {}),
    # Role Management
    ("POST", "/settings/user/role", {"username": "testuser", "role": "admin"}),
    # Group Management
    ("POST", "/settings/group/create", {"id": "test-group", "name": "Test Group"}),
    ("DELETE", "/settings/group/test-group", {}),
    ("POST", "/settings/group/add-member", {"group_id": "test-group", "user_id": "testuser"}),
    ("POST", "/settings/group/remove-member", {"group_id": "test-group", "user_id": "testuser"}),
    # Pack Permissions - Group
    ("POST", "/settings/pack/permission/add", {"pack_id": "test-pack", "group_id": "test-group"}),
    ("POST", "/settings/pack/permission/remove", {"pack_id": "test-pack", "group_id": "test-group"}),
    ("POST", "/settings/pack/test-pack/make-public", {}),
    # Pack Permissions - User
    ("POST", "/settings/pack/user-permission/add", {"pack_id": "test-pack", "user_id": "testuser"}),
    ("POST", "/settings/pack/user-permission/remove", {"pack_id": "test-pack", "user_id": "testuser"}),
    # Pack State
    ("POST", "/settings/pack/test-pack/disable", {}),
    ("POST", "/settings/pack/test-pack/enable", {}),
    # Pack Paths
    ("POST", "/settings/pack-paths/register", {"path": "/tmp/test-pack"}),
    ("DELETE", "/settings/pack-paths/999", {}),
    ("POST", "/settings/pack-paths/999/toggle", {}),
    ("POST", "/settings/pack-paths/browse", {"path": "/"}),
    # Scraper Operations
    ("POST", "/settings/scrape", {}),
    ("POST", "/settings/scrape/lesson1", {}),
    ("POST", "/settings/delete-scraped", {}),
    ("POST", "/settings/delete-scraped/lesson1", {}),
    ("POST", "/settings/segment", {}),
    ("POST", "/settings/segment-row", {"audio_id": "1"}),
    ("POST", "/settings/segment-manual", {}),
    ("POST", "/settings/segment-reset", {}),
]


class TestUnauthenticatedAccess:
    """All admin endpoints reject unauthenticated users."""

    @pytest.mark.parametrize("method,path,data", ADMIN_ENDPOINTS)
    def test_unauthenticated_rejected(
        self, client: TestClient, method: str, path: str, data: dict
    ):
        """Unauthenticated requests redirect to /login."""
        # Ensure client has no session
        assert not client.is_authenticated(), "Client should not have session"

        response = client.request(method, path, data=data)

        # Should redirect to login page
        assert response.status_code in (302, 303), (
            f"Expected redirect (302/303), got {response.status_code} for {method} {path}"
        )
        location = response.headers.get("location", "")
        assert "/login" in location, (
            f"Expected redirect to /login, got '{location}' for {method} {path}"
        )


class TestUnauthorizedAccess:
    """Non-admin users cannot access admin endpoints."""

    @pytest.mark.parametrize(
        "method,path,data",
        [
            # Guest Management
            ("POST", "/settings/cleanup-guests", {}),
            ("POST", "/settings/delete-all-guests", {}),
            # Role Management
            ("POST", "/settings/user/role", {"username": "x", "role": "admin"}),
            # Group Management
            ("DELETE", "/settings/group/test-group", {}),
            ("POST", "/settings/group/add-member", {"group_id": "x", "user_id": "x"}),
            ("POST", "/settings/group/remove-member", {"group_id": "x", "user_id": "x"}),
            # Pack Permissions - Group
            ("POST", "/settings/pack/permission/add", {"pack_id": "x", "group_id": "x"}),
            ("POST", "/settings/pack/permission/remove", {"pack_id": "x", "group_id": "x"}),
            ("POST", "/settings/pack/test-pack/make-public", {}),
            # Pack Permissions - User
            ("POST", "/settings/pack/user-permission/add", {"pack_id": "x", "user_id": "x"}),
            ("POST", "/settings/pack/user-permission/remove", {"pack_id": "x", "user_id": "x"}),
            # Pack State
            ("POST", "/settings/pack/test-pack/disable", {}),
            ("POST", "/settings/pack/test-pack/enable", {}),
            # Pack Paths
            ("POST", "/settings/pack-paths/register", {"path": "/tmp"}),
            ("DELETE", "/settings/pack-paths/999", {}),
            ("POST", "/settings/pack-paths/999/toggle", {}),
            ("POST", "/settings/pack-paths/browse", {"path": "/"}),
            # Scraper Operations
            ("POST", "/settings/scrape", {}),
            ("POST", "/settings/scrape/lesson1", {}),
            ("POST", "/settings/delete-scraped", {}),
            ("POST", "/settings/delete-scraped/lesson1", {}),
            ("POST", "/settings/segment", {}),
            ("POST", "/settings/segment-row", {"audio_id": "1"}),
            ("POST", "/settings/segment-manual", {}),
            ("POST", "/settings/segment-reset", {}),
        ],
    )
    def test_unauthorized_rejected(
        self, authenticated_client: TestClient, method: str, path: str, data: dict
    ):
        """Non-admin users get 403 or redirect to /settings."""
        assert authenticated_client.is_authenticated(), "Client should be authenticated"

        response = authenticated_client.request(method, path, data=data)

        # Should be rejected with 403 or redirect to /settings (not /login)
        assert response.status_code in (303, 403), (
            f"Expected 403 or redirect (303), got {response.status_code} for {method} {path}"
        )
        if response.status_code == 303:
            location = response.headers.get("location", "")
            # Should redirect to /settings, NOT /login (user is authenticated)
            assert "/settings" in location or location == "/settings", (
                f"Expected redirect to /settings, got '{location}' for {method} {path}"
            )


class TestUnauthorizedGroupCreation:
    """Non-admin users cannot create groups - with side-effect verification."""

    def test_create_group_unauthorized_with_side_effect_check(
        self, authenticated_client: TestClient, db_manager: DbManager
    ):
        """Non-admin cannot create groups and group is NOT created."""
        group_id = "hacked-group-test"
        count_before = db_manager.get_group_count()

        response = authenticated_client.post(
            "/settings/group/create",
            data={"id": group_id, "name": "Hacked Group"},
        )

        # Must be rejected
        assert response.status_code in (303, 403), (
            f"Expected 403 or redirect (303), got {response.status_code}"
        )

        # Side effect: group was NOT created
        count_after = db_manager.get_group_count()
        assert count_after == count_before, (
            f"Group count changed from {count_before} to {count_after} - "
            "unauthorized group creation succeeded!"
        )
        assert not db_manager.group_exists(group_id), (
            f"Group '{group_id}' exists - unauthorized group creation succeeded!"
        )


class TestUnauthenticatedGroupCreation:
    """Unauthenticated users cannot create groups - with side-effect verification."""

    def test_create_group_unauthenticated_with_side_effect_check(
        self, client: TestClient, db_manager: DbManager
    ):
        """Unauthenticated user cannot create groups and group is NOT created."""
        assert not client.is_authenticated(), "Client should not be authenticated"

        group_id = "hacked-group-unauth"
        count_before = db_manager.get_group_count()

        response = client.post(
            "/settings/group/create",
            data={"id": group_id, "name": "Hacked Group"},
        )

        # Must redirect to login
        assert response.status_code in (302, 303), (
            f"Expected redirect to login, got {response.status_code}"
        )
        location = response.headers.get("location", "")
        assert "/login" in location, f"Expected redirect to /login, got '{location}'"

        # Side effect: group was NOT created
        count_after = db_manager.get_group_count()
        assert count_after == count_before, (
            f"Group count changed from {count_before} to {count_after} - "
            "unauthenticated group creation succeeded!"
        )
        assert not db_manager.group_exists(group_id), (
            f"Group '{group_id}' exists - unauthenticated group creation succeeded!"
        )
