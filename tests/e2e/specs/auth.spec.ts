import { test, expect, createTestUser, deleteTestUser, login } from '../fixtures/auth';

test.describe('Authentication', () => {
  test.describe('Login Page', () => {
    test('should display login form', async ({ page }) => {
      await page.goto('/login');

      await expect(page.locator('[data-testid="username-input"]')).toBeVisible();
      await expect(page.locator('[data-testid="password-input"]')).toBeVisible();
      await expect(page.locator('[data-testid="login-submit"]')).toBeVisible();
    });

    test('should have links to register and guest', async ({ page }) => {
      await page.goto('/login');

      await expect(page.locator('a[href="/register"]')).toBeVisible();
      await expect(page.locator('a[href="/guest"]')).toBeVisible();
    });

    test('should show error for invalid credentials', async ({ page }) => {
      await page.goto('/login');
      await page.locator('[data-testid="username-input"]').fill('nonexistent');
      await page.locator('[data-testid="password-input"]').fill('wrongpassword');
      await page.locator('[data-testid="login-submit"]').click();

      await expect(page).toHaveURL(/\/login/);

      const errorMessage = page.locator('[data-testid="error-message"]');
      await expect(errorMessage).toBeVisible();
      await expect(errorMessage).toContainText(/invalid|incorrect|wrong|failed/i);
    });

    test('should redirect to home after successful login', async ({ page, testUser }) => {
      await login(page, testUser);
      await expect(page).toHaveURL('/');
    });
  });

  test.describe('Protected Routes', () => {
    test('should redirect to login when not authenticated', async ({ page }) => {
      await page.goto('/study');
      await expect(page).toHaveURL(/\/login/);
    });

    test('should access study page when authenticated', async ({ authenticatedPage }) => {
      await authenticatedPage.goto('/study');
      await expect(authenticatedPage).toHaveURL('/study');
    });

    test('should access progress page when authenticated', async ({ authenticatedPage }) => {
      await authenticatedPage.goto('/progress');
      await expect(authenticatedPage).toHaveURL('/progress');
    });

    test('should access settings page when authenticated', async ({ authenticatedPage }) => {
      await authenticatedPage.goto('/settings');
      await expect(authenticatedPage).toHaveURL('/settings');
    });
  });

  test.describe('Logout', () => {
    test('should clear session and redirect to login', async ({ authenticatedPage }) => {
      await authenticatedPage.goto('/');

      const logoutButton = authenticatedPage.locator(
        '[data-testid="logout-button"], [data-testid="logout-btn"], button:has-text("Logout"), a:has-text("Logout")'
      ).first();

      await expect(logoutButton).toBeVisible();

      await Promise.all([
        authenticatedPage.waitForURL(/\/login/),
        logoutButton.click(),
      ]);

      await authenticatedPage.goto('/study');
      await expect(authenticatedPage).toHaveURL(/\/login/);
    });
  });

  test.describe('Guest Access', () => {
    test('should create guest account', async ({ page }) => {
      await page.goto('/guest');
      await expect(page.locator('[data-testid="guest-submit"]')).toBeVisible();

      await page.locator('[data-testid="guest-submit"]').click();

      await expect(page).toHaveURL('/');
    });
  });
});
