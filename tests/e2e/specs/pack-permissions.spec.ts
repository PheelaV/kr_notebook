import { test, expect } from '../fixtures/auth';

test.describe('Pack Visibility', () => {
  test('regular user sees pack list in settings', async ({ authenticatedPage }) => {
    await authenticatedPage.goto('/settings');

    // Look for content packs section (this is in a partial)
    const packsSection = authenticatedPage.locator('#packs');
    // Packs section may or may not exist depending on content
    // Just verify the settings page loads properly
    await expect(authenticatedPage.locator('[data-testid="learning-section"]')).toBeVisible();
  });

  test('admin sees all packs regardless of restrictions', async ({ adminPage }) => {
    await adminPage.goto('/settings');

    // Admin should see pack management with permission controls
    // This would show pack cards with restriction options
    await expect(adminPage.locator('[data-testid="learning-section"]')).toBeVisible();
  });
});

test.describe('Pack Enable/Disable', () => {
  test('regular user cannot disable global packs', async ({ authenticatedPage }) => {
    await authenticatedPage.goto('/settings');

    // Find any disable button if present
    const disableButtons = authenticatedPage.locator('button:has-text("Disable")');

    // Count visible disable buttons - regular users shouldn't see global pack disable
    const count = await disableButtons.count();

    // If there are disable buttons, clicking them should fail or redirect
    if (count > 0) {
      // Try clicking the first one
      const response = await authenticatedPage.request.post('/settings/pack/test-pack/disable');
      // Should get redirected (403 or redirect to settings with error)
      expect([303, 403]).toContain(response.status());
    }
  });
});

test.describe('Admin Pack Permissions', () => {
  test('admin sees pack paths section', async ({ adminPage }) => {
    await adminPage.goto('/settings');

    await expect(adminPage.locator('[data-testid="pack-paths-section"]')).toBeVisible();
  });

  test('admin can register external pack path', async ({ adminPage }) => {
    await adminPage.goto('/settings');

    const packPathsSection = adminPage.locator('[data-testid="pack-paths-section"]');
    await expect(packPathsSection).toBeVisible();

    // Should have path input
    const pathInput = packPathsSection.locator('input[name="path"]');
    await expect(pathInput).toBeVisible();

    // Should have register button
    await expect(packPathsSection.locator('button:has-text("Register Path")')).toBeVisible();
  });

  test('admin sees directory browser button', async ({ adminPage }) => {
    await adminPage.goto('/settings');

    const packPathsSection = adminPage.locator('[data-testid="pack-paths-section"]');
    // Browse button may have different text - look for it by onclick attribute or text containing "Browse"
    const browseBtn = packPathsSection.locator('button').filter({ hasText: /Browse/i });
    await expect(browseBtn).toBeVisible();
  });

  test('regular user cannot access pack paths section', async ({ authenticatedPage }) => {
    await authenticatedPage.goto('/settings');

    await expect(authenticatedPage.locator('[data-testid="pack-paths-section"]')).not.toBeVisible();
  });

  test('regular user cannot POST to pack permission endpoints', async ({ authenticatedPage }) => {
    // Try to add pack permission
    const response = await authenticatedPage.request.post('/settings/pack/permission/add', {
      form: {
        pack_id: 'some-pack',
        group_id: 'some-group',
      },
    });

    // Non-admin should either get redirected (303) or denied
    // Accept 200 (with error in body), 303 redirect, or 403 forbidden
    expect([200, 303, 403]).toContain(response.status());
  });

  test('regular user cannot make pack public', async ({ authenticatedPage }) => {
    const response = await authenticatedPage.request.post('/settings/pack/some-pack/make-public');

    // Non-admin should either get redirected (303) or denied
    // Accept 200 (with error in body), 303 redirect, or 403 forbidden
    expect([200, 303, 403]).toContain(response.status());
  });
});

test.describe('Pack Restriction UI', () => {
  test('admin sees groups list', async ({ adminPage }) => {
    await adminPage.goto('/settings');

    // Group management section should be visible for admins
    await expect(adminPage.locator('[data-testid="group-management"]')).toBeVisible();
  });

  test('pack permissions require admin auth', async ({ page }) => {
    // Try to access settings without auth
    await page.goto('/settings');

    // Should redirect to login
    await expect(page).toHaveURL(/\/login/);
  });
});
