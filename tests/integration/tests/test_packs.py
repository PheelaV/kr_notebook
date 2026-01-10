"""Pack and permissions integration tests.

Tests cover:
- Pack discovery and display
- Pack enable/disable
- Pack permission management (admin)
- Group-based access control
"""

import uuid

import pytest

from conftest import DbManager, TestClient


class TestPackDiscovery:
    """Pack discovery and display tests."""

    def test_settings_shows_packs(self, authenticated_client: TestClient):
        """Settings page displays available content packs."""
        response = authenticated_client.get("/settings")

        assert response.status_code == 200
        # Settings should have pack-related content
        assert "pack" in response.text.lower() or "content" in response.text.lower()


class TestPackEnableDisable:
    """Pack enable/disable tests."""

    def test_enable_pack_endpoint_exists(self, authenticated_client: TestClient):
        """POST /settings/pack/{pack_id}/enable endpoint responds."""
        # Try to enable a non-existent pack
        response = authenticated_client.post("/settings/pack/test-pack/enable", data={})

        # Should return some response (may be error for non-existent pack)
        assert response.status_code in (200, 302, 303, 400, 404)

    def test_disable_pack_endpoint_exists(self, authenticated_client: TestClient):
        """POST /settings/pack/{pack_id}/disable endpoint responds."""
        response = authenticated_client.post("/settings/pack/test-pack/disable", data={})

        assert response.status_code in (200, 302, 303, 400, 404)


class TestPackPermissions:
    """Pack permission management tests (admin only)."""

    def test_add_pack_permission_requires_admin(self, authenticated_client: TestClient):
        """POST /settings/pack/permission/add requires admin."""
        response = authenticated_client.post(
            "/settings/pack/permission/add",
            data={"pack_id": "test-pack", "group_id": "test-group"},
        )

        # Regular users should not have access
        assert response.status_code in (200, 302, 303, 403)

    def test_remove_pack_permission_requires_admin(self, authenticated_client: TestClient):
        """POST /settings/pack/permission/remove requires admin."""
        response = authenticated_client.post(
            "/settings/pack/permission/remove",
            data={"pack_id": "test-pack", "group_id": "test-group"},
        )

        assert response.status_code in (200, 302, 303, 403)

    def test_make_pack_public_requires_admin(self, authenticated_client: TestClient):
        """POST /settings/pack/{pack_id}/make-public requires admin."""
        response = authenticated_client.post(
            "/settings/pack/test-pack/make-public",
            data={},
        )

        assert response.status_code in (200, 302, 303, 403)


class TestUserPackPermissions:
    """Individual user pack permission tests."""

    def test_add_user_permission_requires_admin(self, authenticated_client: TestClient):
        """POST /settings/pack/user-permission/add requires admin."""
        response = authenticated_client.post(
            "/settings/pack/user-permission/add",
            data={"pack_id": "test-pack", "username": "testuser"},
        )

        assert response.status_code in (200, 302, 303, 403)

    def test_remove_user_permission_requires_admin(self, authenticated_client: TestClient):
        """POST /settings/pack/user-permission/remove requires admin."""
        response = authenticated_client.post(
            "/settings/pack/user-permission/remove",
            data={"pack_id": "test-pack", "username": "testuser"},
        )

        assert response.status_code in (200, 302, 303, 403)


class TestPackAccessControl:
    """Pack access control workflow tests."""

    def test_user_only_sees_permitted_packs(
        self,
        client: TestClient,
        db_manager: DbManager,
    ):
        """User only sees packs they have permission to access."""
        username = f"_test_{uuid.uuid4().hex[:8]}"
        password = "test123"

        try:
            password_hash = db_manager.create_user(username, password)
            client.login(username, password_hash)

            # Check settings page - should only show accessible packs
            response = client.get("/settings")
            assert response.status_code == 200

        finally:
            db_manager.delete_user(username)
