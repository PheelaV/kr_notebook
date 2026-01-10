import { test, expect, createGroup, deleteGroup } from '../fixtures/auth';

test.describe('Group Management', () => {
  test('admin sees group creation form', async ({ adminPage }) => {
    await adminPage.goto('/settings');

    const groupSection = adminPage.locator('[data-testid="group-management"]');
    await expect(groupSection).toBeVisible();

    // Should have form inputs
    await expect(groupSection.locator('input[name="id"]')).toBeVisible();
    await expect(groupSection.locator('input[name="name"]')).toBeVisible();
    await expect(groupSection.locator('button:has-text("Create Group")')).toBeVisible();
  });

  test('admin can create a group', async ({ adminPage }) => {
    await adminPage.goto('/settings');

    const groupSection = adminPage.locator('[data-testid="group-management"]');
    const groupId = `test-group-${Date.now()}`;

    // Fill in the form
    await groupSection.locator('input[name="id"]').fill(groupId);
    await groupSection.locator('input[name="name"]').fill('Test Group');

    // Submit via HTMX (click the button)
    await groupSection.locator('button:has-text("Create Group")').click();

    // Wait for HTMX to complete
    await adminPage.waitForTimeout(500);

    // The group should appear in the list
    const groupsList = groupSection.locator('#groups-list');
    await expect(groupsList.locator(`text=${groupId}`)).toBeVisible({ timeout: 5000 });
  });

  test('admin can delete a group', async ({ adminPage, dataDir }) => {
    // SETUP: Create group via db-manager
    const groupId = `delete-test-${Date.now()}`;
    createGroup(groupId, 'To Delete', dataDir);

    await adminPage.goto('/settings');

    const groupSection = adminPage.locator('[data-testid="group-management"]');

    // Verify group exists in UI
    const groupCard = groupSection.locator(`[id="group-${groupId}"]`);
    await expect(groupCard).toBeVisible({ timeout: 5000 });

    // Find delete button - no fallback, it must exist
    const deleteBtn = groupCard.locator('button:has-text("Delete")');
    await expect(deleteBtn).toBeVisible({ timeout: 5000 });

    // Set up dialog handler for the hx-confirm dialog
    adminPage.on('dialog', async (dialog) => {
      await dialog.accept();
    });

    await deleteBtn.click();
    // Wait for HTMX to process the delete
    await adminPage.waitForTimeout(1500);

    // VERIFY EFFECT: Group is gone from UI
    await expect(groupCard).not.toBeVisible({ timeout: 5000 });
  });

  test('regular user cannot access group management', async ({ authenticatedPage }) => {
    await authenticatedPage.goto('/settings');

    // Group management section should not be visible
    await expect(authenticatedPage.locator('[data-testid="group-management"]')).not.toBeVisible();
  });

  test('regular user cannot POST to group endpoints', async ({ authenticatedPage }) => {
    // Try to POST to create group endpoint as non-admin
    const response = await authenticatedPage.request.post('/settings/group/create', {
      form: {
        id: `hacked-group-${Date.now()}`,
        name: 'Hacked Group',
      },
    });

    // VERIFY: Request was denied with 403 Forbidden
    // Side-effect verification (group not created) should be in integration tests
    expect(response.status()).toBe(403);
  });
});

test.describe('Group Membership', () => {
  test('admin sees add member form in group card', async ({ adminPage, dataDir }) => {
    // SETUP: Create group via db-manager
    const groupId = `member-test-${Date.now()}`;
    createGroup(groupId, 'Member Test', dataDir);

    await adminPage.goto('/settings');

    const groupSection = adminPage.locator('[data-testid="group-management"]');

    // The group card should be visible
    const groupCard = groupSection.locator(`[id="group-${groupId}"]`);
    await expect(groupCard).toBeVisible({ timeout: 5000 });

    // Check if there's any form of member management UI
    // This could be input, select, or button depending on implementation
    const hasUserInput = await groupCard.locator('input[name="user_id"]').count();
    const hasUserSelect = await groupCard.locator('select[name="user_id"]').count();
    const hasAddMemberBtn = await groupCard.locator('button:has-text("Add")').count();

    // At least one form of member management should exist
    // FIX: Changed from >= 0 (always true) to > 0 (at least one element exists)
    expect(hasUserInput + hasUserSelect + hasAddMemberBtn).toBeGreaterThan(0);

    // CLEANUP
    deleteGroup(groupId, dataDir);
  });
});
