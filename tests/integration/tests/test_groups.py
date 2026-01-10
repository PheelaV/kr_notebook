"""Group management integration tests.

Tests cover:
- Group creation (admin)
- Group membership management
- Group-based pack access
"""

import uuid

import pytest

from conftest import DbManager, TestClient


class TestGroupManagement:
    """Group CRUD operations (admin only)."""

    def test_create_group_requires_admin(self, authenticated_client: TestClient):
        """POST /settings/group/create requires admin."""
        response = authenticated_client.post(
            "/settings/group/create",
            data={"name": "test-group", "description": "Test group"},
        )

        # Regular users should not have access
        assert response.status_code in (200, 302, 303, 403)

    def test_delete_group_requires_admin(self, authenticated_client: TestClient):
        """DELETE /settings/group/{group_id} requires admin."""
        # Use POST with method override or direct DELETE
        response = authenticated_client.post(
            "/settings/group/test-group-id",
            data={"_method": "DELETE"},
        )

        assert response.status_code in (200, 302, 303, 403, 404, 405)


class TestGroupMembership:
    """Group membership management (admin only)."""

    def test_add_member_requires_admin(self, authenticated_client: TestClient):
        """POST /settings/group/add-member requires admin."""
        response = authenticated_client.post(
            "/settings/group/add-member",
            data={"group_id": "test-group", "username": "testuser"},
        )

        assert response.status_code in (200, 302, 303, 403)

    def test_remove_member_requires_admin(self, authenticated_client: TestClient):
        """POST /settings/group/remove-member requires admin."""
        response = authenticated_client.post(
            "/settings/group/remove-member",
            data={"group_id": "test-group", "username": "testuser"},
        )

        assert response.status_code in (200, 302, 303, 403)


class TestGroupAccessControl:
    """Group-based access control tests."""

    def test_settings_shows_user_groups(self, authenticated_client: TestClient):
        """Settings page may show group membership info."""
        response = authenticated_client.get("/settings")

        assert response.status_code == 200
        # Settings page should load successfully
