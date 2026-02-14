import { test, expect, createGroup, deleteGroup } from '../fixtures/auth';

test.describe('Group Management', () => {
  test('admin sees group creation form', async ({ adminPage }) => {
    await adminPage.goto('/settings');

    const groupSection = adminPage.locator('[data-testid="group-management"]');
    await expect(groupSection).toBeVisible();

    await expect(groupSection.locator('input[name="id"]')).toBeVisible();
    await expect(groupSection.locator('input[name="name"]')).toBeVisible();
    await expect(groupSection.locator('button:has-text("Create Group")')).toBeVisible();
  });

  test('admin can create a group', async ({ adminPage }) => {
    await adminPage.goto('/settings');

    const groupSection = adminPage.locator('[data-testid="group-management"]');
    const groupId = `test-group-${Date.now()}`;

    await groupSection.locator('input[name="id"]').fill(groupId);
    await groupSection.locator('input[name="name"]').fill('Test Group');

    await groupSection.locator('button:has-text("Create Group")').click();

    // Wait for HTMX to complete instead of waitForTimeout
    const groupsList = groupSection.locator('#groups-list');
    await expect(groupsList.locator(`text=${groupId}`)).toBeVisible();
  });

  test('admin can delete a group', async ({ adminPage, dataDir }) => {
    const groupId = `delete-test-${Date.now()}`;
    createGroup(groupId, 'To Delete', dataDir);

    await adminPage.goto('/settings');

    const groupSection = adminPage.locator('[data-testid="group-management"]');

    const groupCard = groupSection.locator(`[id="group-${groupId}"]`);
    await expect(groupCard).toBeVisible();

    const deleteBtn = groupCard.locator('button:has-text("Delete")');
    await expect(deleteBtn).toBeVisible();

    adminPage.on('dialog', async (dialog) => {
      await dialog.accept();
    });

    await deleteBtn.click();

    // Wait for HTMX to process the delete instead of waitForTimeout
    await expect(groupCard).not.toBeVisible();
  });

  test('regular user cannot access group management', async ({ authenticatedPage }) => {
    await authenticatedPage.goto('/settings');

    await expect(authenticatedPage.locator('[data-testid="group-management"]')).not.toBeVisible();
  });

  test('regular user cannot POST to group endpoints', async ({ authenticatedPage }) => {
    const response = await authenticatedPage.request.post('/settings/group/create', {
      form: {
        id: `hacked-group-${Date.now()}`,
        name: 'Hacked Group',
      },
    });

    expect(response.status()).toBe(403);
  });
});

test.describe('Group Membership', () => {
  test('admin sees add member form in group card', async ({ adminPage, dataDir }) => {
    const groupId = `member-test-${Date.now()}`;
    createGroup(groupId, 'Member Test', dataDir);

    await adminPage.goto('/settings');

    const groupSection = adminPage.locator('[data-testid="group-management"]');

    const groupCard = groupSection.locator(`[id="group-${groupId}"]`);
    await expect(groupCard).toBeVisible();

    const hasUserInput = await groupCard.locator('input[name="user_id"]').count();
    const hasUserSelect = await groupCard.locator('select[name="user_id"]').count();
    const hasAddMemberBtn = await groupCard.locator('button:has-text("Add")').count();

    expect(hasUserInput + hasUserSelect + hasAddMemberBtn).toBeGreaterThan(0);

    deleteGroup(groupId, dataDir);
  });
});
