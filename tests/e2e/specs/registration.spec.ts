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
    await page.goto('/register');

    await page.locator('[data-testid="register-username"]').fill(testUser.username);
    await page.locator('[data-testid="register-password"]').fill('newpassword123');
    await page.locator('[data-testid="register-confirm"]').fill('newpassword123');
    await page.locator('[data-testid="register-submit"]').click();

    const errorMessage = page.locator('[data-testid="register-error"]');
    await expect(errorMessage).toBeVisible();
    await expect(errorMessage).toContainText(/already exists|taken|in use|duplicate/i);

    await expect(page).toHaveURL(/\/register/);
  });

  test('should redirect to home after successful registration', async ({ page }) => {
    const uniqueUsername = `_test_reg_${Date.now()}`;

    await page.goto('/register');
    await page.locator('[data-testid="register-username"]').fill(uniqueUsername);
    await page.locator('[data-testid="register-password"]').fill('testpass123');
    await page.locator('[data-testid="register-confirm"]').fill('testpass123');

    await Promise.all([
      page.waitForURL('/'),
      page.locator('[data-testid="register-submit"]').click(),
    ]);

    await expect(page).toHaveURL('/');
  });

  test('should show validation error for short username', async ({ page }) => {
    await page.goto('/register');

    await page.locator('[data-testid="register-username"]').fill('ab');
    await page.locator('[data-testid="register-password"]').fill('testpass123');
    await page.locator('[data-testid="register-confirm"]').fill('testpass123');
    await page.locator('[data-testid="register-submit"]').click();

    await expect(page).toHaveURL(/\/register/);

    const usernameInput = page.locator('[data-testid="register-username"]');
    const validationMessage = await usernameInput.evaluate(
      (el: HTMLInputElement) => el.validationMessage
    );
    expect(validationMessage).toBeTruthy();
  });

  test('should require password confirmation to match', async ({ page }) => {
    await page.goto('/register');

    await page.locator('[data-testid="register-username"]').fill(`_test_mismatch_${Date.now()}`);
    await page.locator('[data-testid="register-password"]').fill('password123');
    await page.locator('[data-testid="register-confirm"]').fill('differentpassword');
    await page.locator('[data-testid="register-submit"]').click();

    await expect(page).toHaveURL(/\/register/);
  });
});
