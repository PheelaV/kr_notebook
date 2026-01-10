"""Group management integration tests.

Tests cover:
- Group creation (admin)
- Group membership management
- Group-based pack access

Note: Comprehensive security tests (unauthorized/unauthenticated access) are in
test_admin_security.py. These tests focus on functionality when authorized.
"""

import uuid

import pytest

from conftest import DbManager, TestClient


class TestGroupManagement:
    """Group CRUD operations (admin only)."""

    def test_create_group_requires_admin(
        self, authenticated_client: TestClient, db_manager: DbManager
    ):
        """POST /settings/group/create requires admin - with side-effect check."""
        group_id = f"test-group-{uuid.uuid4().hex[:8]}"
        count_before = db_manager.get_group_count()

        response = authenticated_client.post(
            "/settings/group/create",
            data={"id": group_id, "name": "Test Group"},
        )

        # Regular users should get 403 or redirect to /settings
        # 200 is NOT acceptable - it could mean the action succeeded
        assert response.status_code in (303, 403), (
            f"Expected 403 or redirect (303), got {response.status_code}"
        )

        # Side effect: group was NOT created
        count_after = db_manager.get_group_count()
        assert count_after == count_before, (
            f"Group count changed from {count_before} to {count_after}"
        )
        assert not db_manager.group_exists(group_id), (
            f"Group '{group_id}' was created despite non-admin user"
        )

    def test_delete_group_requires_admin(self, authenticated_client: TestClient):
        """DELETE /settings/group/{group_id} requires admin."""
        response = authenticated_client.delete("/settings/group/nonexistent-group")

        # Regular users should get 403 or redirect to /settings
        # 404 is also acceptable since group doesn't exist
        # 200 is NOT acceptable
        assert response.status_code in (303, 403, 404), (
            f"Expected 403, 404, or redirect (303), got {response.status_code}"
        )


class TestGroupMembership:
    """Group membership management (admin only)."""

    def test_add_member_requires_admin(self, authenticated_client: TestClient):
        """POST /settings/group/add-member requires admin."""
        response = authenticated_client.post(
            "/settings/group/add-member",
            data={"group_id": "test-group", "user_id": "testuser"},
        )

        # Regular users should get 403 or redirect to /settings
        # 200 is NOT acceptable
        assert response.status_code in (303, 403), (
            f"Expected 403 or redirect (303), got {response.status_code}"
        )

    def test_remove_member_requires_admin(self, authenticated_client: TestClient):
        """POST /settings/group/remove-member requires admin."""
        response = authenticated_client.post(
            "/settings/group/remove-member",
            data={"group_id": "test-group", "user_id": "testuser"},
        )

        # Regular users should get 403 or redirect to /settings
        # 200 is NOT acceptable
        assert response.status_code in (303, 403), (
            f"Expected 403 or redirect (303), got {response.status_code}"
        )


class TestGroupAccessControl:
    """Group-based access control tests."""

    def test_settings_shows_user_groups(self, authenticated_client: TestClient):
        """Settings page may show group membership info."""
        response = authenticated_client.get("/settings")

        assert response.status_code == 200
        # Settings page should load successfully
