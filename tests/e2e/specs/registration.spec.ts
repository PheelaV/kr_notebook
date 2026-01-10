import { test, expect } from '../fixtures/auth';

test.describe('Registration', () => {
  test('should display registration form', async ({ page }) => {
    await page.goto('/register');

    await expect(page.locator('[data-testid="register-username"]')).toBeVisible();
    await expect(page.locator('[data-testid="register-password"]')).toBeVisible();
    await expect(page.locator('[data-testid="register-confirm"]')).toBeVisible();
    await expect(page.locator('[data-testid="register-submit"]')).toBeVisible();
  });

  test('should have link to login page', async ({ page }) => {
    await page.goto('/register');

    await expect(page.locator('a[href="/login"]')).toBeVisible();
  });

  test('should show error for duplicate username', async ({ page, testUser }) => {
    // testUser is already created by the fixture
    await page.goto('/register');

    // Try to register with the same username
    await page.fill('[data-testid="register-username"]', testUser.username);
    await page.fill('[data-testid="register-password"]', 'newpassword123');
    await page.fill('[data-testid="register-confirm"]', 'newpassword123');
    await page.click('[data-testid="register-submit"]');

    // Should show error message with meaningful text about duplicate/exists
    const errorMessage = page.locator('[data-testid="register-error"]');
    await expect(errorMessage).toBeVisible();
    await expect(errorMessage).toContainText(/already exists|taken|in use|duplicate/i);

    // Should stay on register page
    await expect(page).toHaveURL(/\/register/);
  });

  test('should redirect to home after successful registration', async ({ page }) => {
    const uniqueUsername = `_test_reg_${Date.now()}`;

    await page.goto('/register');
    await page.fill('[data-testid="register-username"]', uniqueUsername);
    await page.fill('[data-testid="register-password"]', 'testpass123');
    await page.fill('[data-testid="register-confirm"]', 'testpass123');

    await Promise.all([
      page.waitForURL('/'),
      page.click('[data-testid="register-submit"]'),
    ]);

    // Should be on home page
    await expect(page).toHaveURL('/');
  });

  test('should show validation error for short username', async ({ page }) => {
    await page.goto('/register');

    // Try with a 2-character username (too short)
    await page.fill('[data-testid="register-username"]', 'ab');
    await page.fill('[data-testid="register-password"]', 'testpass123');
    await page.fill('[data-testid="register-confirm"]', 'testpass123');
    await page.click('[data-testid="register-submit"]');

    // Browser validation should prevent submission (pattern requires 3-32 chars)
    // The form won't submit so we should still be on register page
    await expect(page).toHaveURL(/\/register/);

    // VERIFY: Browser validation message is shown (for pattern mismatch)
    const usernameInput = page.locator('[data-testid="register-username"]');
    const validationMessage = await usernameInput.evaluate(
      (el: HTMLInputElement) => el.validationMessage
    );
    expect(validationMessage).toBeTruthy();
  });

  test('should require password confirmation to match', async ({ page }) => {
    await page.goto('/register');

    await page.fill('[data-testid="register-username"]', `_test_mismatch_${Date.now()}`);
    await page.fill('[data-testid="register-password"]', 'password123');
    await page.fill('[data-testid="register-confirm"]', 'differentpassword');
    await page.click('[data-testid="register-submit"]');

    // VERIFY: Form doesn't submit (stays on register page)
    await expect(page).toHaveURL(/\/register/);

    // NOTE: Currently no user feedback is shown for password mismatch.
    // This is a UX gap - users don't know why the form won't submit.
    // Future improvement: Add client-side validation message for password mismatch.
  });
});
