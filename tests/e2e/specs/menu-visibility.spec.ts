import { test, expect } from '../fixtures/auth';

test.describe('Navigation Menu', () => {
  test('authenticated user sees logout button', async ({ authenticatedPage }) => {
    await authenticatedPage.goto('/');
    await expect(authenticatedPage.locator('[data-testid="logout-btn"]')).toBeVisible();
  });

  test('authenticated user sees settings link', async ({ authenticatedPage }) => {
    await authenticatedPage.goto('/');
    await expect(authenticatedPage.locator('[data-testid="nav-settings"]')).toBeVisible();
  });

  test('unauthenticated user redirected to login', async ({ page }) => {
    await page.goto('/');
    await expect(page).toHaveURL(/\/login/);
  });
});

test.describe('Settings Page Sections - Regular User', () => {
  test('regular user sees basic sections', async ({ authenticatedPage }) => {
    await authenticatedPage.goto('/settings');

    // Should see appearance, learning, and data sections
    await expect(authenticatedPage.locator('[data-testid="appearance-section"]')).toBeVisible();
    await expect(authenticatedPage.locator('[data-testid="learning-section"]')).toBeVisible();
    await expect(authenticatedPage.locator('[data-testid="data-section"]')).toBeVisible();
  });

  test('regular user does NOT see admin sections', async ({ authenticatedPage }) => {
    await authenticatedPage.goto('/settings');

    // Admin-only sections should not be visible
    await expect(authenticatedPage.locator('[data-testid="user-management"]')).not.toBeVisible();
    await expect(authenticatedPage.locator('[data-testid="group-management"]')).not.toBeVisible();
    await expect(authenticatedPage.locator('[data-testid="guest-management"]')).not.toBeVisible();
    await expect(authenticatedPage.locator('[data-testid="pack-paths-section"]')).not.toBeVisible();
  });
});

test.describe('Settings Page Sections - Admin User', () => {
  test('admin user sees all sections', async ({ adminPage }) => {
    await adminPage.goto('/settings');

    // Basic sections
    await expect(adminPage.locator('[data-testid="appearance-section"]')).toBeVisible();
    await expect(adminPage.locator('[data-testid="learning-section"]')).toBeVisible();
    await expect(adminPage.locator('[data-testid="data-section"]')).toBeVisible();

    // Admin-only sections
    await expect(adminPage.locator('[data-testid="user-management"]')).toBeVisible();
    await expect(adminPage.locator('[data-testid="group-management"]')).toBeVisible();
    await expect(adminPage.locator('[data-testid="guest-management"]')).toBeVisible();
    await expect(adminPage.locator('[data-testid="pack-paths-section"]')).toBeVisible();
  });

  test('admin sees user management section with user list', async ({ adminPage }) => {
    await adminPage.goto('/settings');

    const userManagement = adminPage.locator('[data-testid="user-management"]');
    await expect(userManagement).toBeVisible();

    // Should have user search input
    await expect(userManagement.locator('#userSearchInput')).toBeVisible();
    // Should have user list
    await expect(userManagement.locator('#userList')).toBeVisible();
  });

  test('admin sees group management section', async ({ adminPage }) => {
    await adminPage.goto('/settings');

    const groupManagement = adminPage.locator('[data-testid="group-management"]');
    await expect(groupManagement).toBeVisible();

    // Should have group creation form
    await expect(groupManagement.locator('input[name="id"]')).toBeVisible();
    await expect(groupManagement.locator('input[name="name"]')).toBeVisible();
  });
});

test.describe('Home Page Elements', () => {
  test('authenticated user sees due count', async ({ authenticatedPage }) => {
    await authenticatedPage.goto('/');
    await expect(authenticatedPage.locator('[data-testid="due-count"]')).toBeVisible();
  });

  test('authenticated user sees study button when cards due', async ({ authenticatedPage, testUser }) => {
    await authenticatedPage.goto('/');

    // Either start study or practice button should be visible
    const startStudy = authenticatedPage.locator('[data-testid="start-study-btn"]');
    const practice = authenticatedPage.locator('[data-testid="practice-btn"]');

    const hasStudy = await startStudy.isVisible().catch(() => false);
    const hasPractice = await practice.isVisible().catch(() => false);

    expect(hasStudy || hasPractice).toBeTruthy();
  });
});
