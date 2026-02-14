import { test as base, expect, Page } from '@playwright/test';
import * as crypto from 'crypto';

/**
 * Fresh install tests verify that:
 * 1. A default admin user is created on first startup
 * 2. Admin can log in with the test password
 * 3. Admin has proper admin privileges (sees admin sections)
 */

const TEST_ADMIN_PASSWORD = 'e2e_test_admin_pwd';
const ADMIN_USERNAME = 'admin';

function computePasswordHash(password: string, username: string): string {
  const combined = `${password}:${username}`;
  return crypto.createHash('sha256').update(combined).digest('hex');
}

async function loginAsAdmin(page: Page, password: string = TEST_ADMIN_PASSWORD): Promise<void> {
  await page.goto('/login', { waitUntil: 'domcontentloaded' });
  await page.locator('[data-testid="username-input"]').fill(ADMIN_USERNAME);
  await page.locator('[data-testid="password-input"]').fill(password);

  await Promise.all([
    page.waitForURL('/'),
    page.locator('[data-testid="login-submit"]').click(),
  ]);
}

const test = base;

test.describe('Fresh Installation', () => {
  test.describe('Default Admin User', () => {
    test('admin user is created on fresh install', async ({ page }) => {
      await loginAsAdmin(page);

      await expect(page).toHaveURL('/');
    });

    test('admin user has admin role', async ({ page }) => {
      await loginAsAdmin(page);

      await page.goto('/settings');
      await expect(page).toHaveURL('/settings');

      await expect(page.locator('[data-testid="user-management"]')).toBeVisible();
      await expect(page.locator('[data-testid="group-management"]')).toBeVisible();
      await expect(page.locator('[data-testid="guest-management"]')).toBeVisible();
      await expect(page.locator('[data-testid="pack-paths-section"]')).toBeVisible();
    });

    test('admin can access all protected routes', async ({ page }) => {
      await loginAsAdmin(page);

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
      const uniqueUsername = `test_${Date.now()}`;
      const password = 'testpassword123';

      await page.goto('/register');

      await page.locator('[data-testid="register-username"]').fill(uniqueUsername);
      await page.locator('[data-testid="register-password"]').fill(password);
      await page.locator('[data-testid="register-confirm"]').fill(password);

      await Promise.all([
        page.waitForURL('/'),
        page.locator('[data-testid="register-submit"]').click(),
      ]);

      await expect(page).toHaveURL('/');
    });

    test('newly registered user does not have admin privileges', async ({ page }) => {
      const uniqueUsername = `nonadmin_${Date.now()}`;
      const password = 'testpassword123';

      await page.goto('/register');
      await page.locator('[data-testid="register-username"]').fill(uniqueUsername);
      await page.locator('[data-testid="register-password"]').fill(password);
      await page.locator('[data-testid="register-confirm"]').fill(password);

      await Promise.all([
        page.waitForURL('/'),
        page.locator('[data-testid="register-submit"]').click(),
      ]);

      await page.goto('/settings');

      await expect(page.locator('[data-testid="user-management"]')).not.toBeVisible();
      await expect(page.locator('[data-testid="group-management"]')).not.toBeVisible();
      await expect(page.locator('[data-testid="guest-management"]')).not.toBeVisible();
      await expect(page.locator('[data-testid="pack-paths-section"]')).not.toBeVisible();
    });
  });
});
