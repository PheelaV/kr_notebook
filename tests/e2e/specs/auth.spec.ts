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
      await page.fill('[data-testid="username-input"]', 'nonexistent');
      await page.fill('[data-testid="password-input"]', 'wrongpassword');
      await page.click('[data-testid="login-submit"]');

      // Should stay on login page with error
      await expect(page).toHaveURL(/\/login/);

      // VERIFY: Error message is visible with meaningful content
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
      // Start from home page
      await authenticatedPage.goto('/');

      // Find logout button - check multiple possible locations/testids
      const logoutButton = authenticatedPage.locator(
        '[data-testid="logout-button"], [data-testid="logout-btn"], button:has-text("Logout"), a:has-text("Logout")'
      ).first();

      // VERIFY: Logout button exists and is accessible
      await expect(logoutButton).toBeVisible({ timeout: 5000 });

      // Click logout
      await Promise.all([
        authenticatedPage.waitForURL(/\/login/),
        logoutButton.click(),
      ]);

      // VERIFY: Session is cleared - accessing protected route redirects to login
      await authenticatedPage.goto('/study');
      await expect(authenticatedPage).toHaveURL(/\/login/);
    });
  });

  test.describe('Guest Access', () => {
    test('should create guest account', async ({ page }) => {
      await page.goto('/guest');
      await expect(page.locator('[data-testid="guest-submit"]')).toBeVisible();

      await page.click('[data-testid="guest-submit"]');

      // Should redirect to home as authenticated guest
      await expect(page).toHaveURL('/');
    });
  });
});
