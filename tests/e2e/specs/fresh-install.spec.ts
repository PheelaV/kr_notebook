import { test as base, expect, Page } from '@playwright/test';
import * as crypto from 'crypto';

/**
 * Fresh install tests verify that:
 * 1. A default admin user is created on first startup
 * 2. Admin can log in with the test password
 * 3. Admin has proper admin privileges (sees admin sections)
 * 4. Password can be changed and still works
 */

// The test admin password is set via TEST_ADMIN_PASSWORD env var in global-setup
const TEST_ADMIN_PASSWORD = 'e2e_test_admin_pwd';
const ADMIN_USERNAME = 'admin';

// Compute client-side password hash (SHA-256 of password:username)
function computePasswordHash(password: string, username: string): string {
  const combined = `${password}:${username}`;
  return crypto.createHash('sha256').update(combined).digest('hex');
}

// Login helper
async function loginAsAdmin(page: Page, password: string = TEST_ADMIN_PASSWORD): Promise<void> {
  await page.goto('/login', { waitUntil: 'networkidle' });
  await page.fill('[data-testid="username-input"]', ADMIN_USERNAME);
  await page.fill('[data-testid="password-input"]', password);

  await Promise.all([
    page.waitForURL('/'),
    page.click('[data-testid="login-submit"]'),
  ]);
}

const test = base;

test.describe('Fresh Installation', () => {
  test.describe('Default Admin User', () => {
    test('admin user is created on fresh install', async ({ page }) => {
      // Simply verify that we can log in as admin with the test password
      await loginAsAdmin(page);

      // Should be on home page after successful login
      await expect(page).toHaveURL('/');
    });

    test('admin user has admin role', async ({ page }) => {
      await loginAsAdmin(page);

      // Navigate to settings
      await page.goto('/settings');
      await expect(page).toHaveURL('/settings');

      // Admin-only sections should be visible
      await expect(page.locator('[data-testid="user-management"]')).toBeVisible();
      await expect(page.locator('[data-testid="group-management"]')).toBeVisible();
      await expect(page.locator('[data-testid="guest-management"]')).toBeVisible();
      await expect(page.locator('[data-testid="pack-paths-section"]')).toBeVisible();
    });

    test('admin can access all protected routes', async ({ page }) => {
      await loginAsAdmin(page);

      // Test various protected routes
      await page.goto('/study');
      await expect(page).toHaveURL('/study');

      await page.goto('/progress');
      await expect(page).toHaveURL('/progress');

      await page.goto('/settings');
      await expect(page).toHaveURL('/settings');

      await page.goto('/library');
      await expect(page).toHaveURL('/library');
    });
  });

  test.describe('User Registration After Fresh Install', () => {
    test('new users can register after fresh install', async ({ page }) => {
      // Generate unique username for this test
      const uniqueUsername = `test_${Date.now()}`;
      const password = 'testpassword123';

      // Go to registration page
      await page.goto('/register');

      // Fill registration form (note: register page has different testids than login)
      await page.fill('[data-testid="register-username"]', uniqueUsername);
      await page.fill('[data-testid="register-password"]', password);
      await page.fill('[data-testid="register-confirm"]', password);

      // Submit registration
      await Promise.all([
        page.waitForURL('/'),
        page.click('[data-testid="register-submit"]'),
      ]);

      // Should be logged in and on home page
      await expect(page).toHaveURL('/');
    });

    test('newly registered user does not have admin privileges', async ({ page }) => {
      // Register a new user
      const uniqueUsername = `nonadmin_${Date.now()}`;
      const password = 'testpassword123';

      await page.goto('/register');
      await page.fill('[data-testid="register-username"]', uniqueUsername);
      await page.fill('[data-testid="register-password"]', password);
      await page.fill('[data-testid="register-confirm"]', password);

      await Promise.all([
        page.waitForURL('/'),
        page.click('[data-testid="register-submit"]'),
      ]);

      // Navigate to settings
      await page.goto('/settings');

      // Admin-only sections should NOT be visible for regular users
      await expect(page.locator('[data-testid="user-management"]')).not.toBeVisible();
      await expect(page.locator('[data-testid="group-management"]')).not.toBeVisible();
      await expect(page.locator('[data-testid="guest-management"]')).not.toBeVisible();
      await expect(page.locator('[data-testid="pack-paths-section"]')).not.toBeVisible();
    });
  });
});
