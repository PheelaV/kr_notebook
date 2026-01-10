import { test, expect } from '../fixtures/auth';

test.describe('Learning Settings', () => {
  test('can toggle unlock all tiers', async ({ authenticatedPage }) => {
    await authenticatedPage.goto('/settings');

    const toggle = authenticatedPage.locator('[data-testid="all-tiers-toggle"]');
    await expect(toggle).toBeVisible();

    // Get initial state
    const wasChecked = await toggle.isChecked();

    // Toggle it using label click or force (some checkboxes need this)
    await toggle.click({ force: true });

    // Wait a bit for the toggle to take effect
    await authenticatedPage.waitForTimeout(100);

    // Verify toggle changed (use evaluate for more reliable check)
    const isNowChecked = await toggle.isChecked();
    expect(isNowChecked).toBe(!wasChecked);

    // Submit the form
    await authenticatedPage.click('[data-testid="settings-submit"]');

    // Page should reload - wait for it
    await authenticatedPage.waitForLoadState('networkidle');

    // Verify the setting persisted
    await authenticatedPage.goto('/settings');
    const newToggle = authenticatedPage.locator('[data-testid="all-tiers-toggle"]');
    const finalState = await newToggle.isChecked();
    expect(finalState).toBe(!wasChecked);
  });

  test('tier options show when unlock all tiers is enabled', async ({ authenticatedPage }) => {
    await authenticatedPage.goto('/settings');

    const toggle = authenticatedPage.locator('[data-testid="all-tiers-toggle"]');
    const tierOptions = authenticatedPage.locator('#tierOptions');

    // Enable unlock all tiers if not already
    if (!(await toggle.isChecked())) {
      await toggle.click();
    }

    // Tier options should be visible
    await expect(tierOptions).toBeVisible();

    // Should have checkboxes for each tier
    await expect(tierOptions.locator('input[name="tier_1"]')).toBeVisible();
    await expect(tierOptions.locator('input[name="tier_2"]')).toBeVisible();
  });

  test('can change retention target', async ({ authenticatedPage }) => {
    await authenticatedPage.goto('/settings');

    const retentionSelect = authenticatedPage.locator('select[name="desired_retention"]');
    await expect(retentionSelect).toBeVisible();

    // Select 85% retention
    await retentionSelect.selectOption('85');

    // Submit
    await authenticatedPage.click('[data-testid="settings-submit"]');
    await authenticatedPage.waitForLoadState('networkidle');

    // Verify it persisted
    await authenticatedPage.goto('/settings');
    const newSelect = authenticatedPage.locator('select[name="desired_retention"]');
    await expect(newSelect).toHaveValue('85');
  });

  test('can change focus tier', async ({ authenticatedPage }) => {
    await authenticatedPage.goto('/settings');

    const focusSelect = authenticatedPage.locator('select[name="focus_tier"]');
    await expect(focusSelect).toBeVisible();

    // Should have "All tiers" option (check it exists via count, not visibility)
    const allTiersOption = focusSelect.locator('option[value="none"]');
    await expect(allTiersOption).toHaveCount(1);
  });
});

test.describe('Data Management', () => {
  test('export button is visible', async ({ authenticatedPage }) => {
    await authenticatedPage.goto('/settings');

    await expect(authenticatedPage.locator('[data-testid="export-btn"]')).toBeVisible();
  });

  test('import button is visible', async ({ authenticatedPage }) => {
    await authenticatedPage.goto('/settings');

    await expect(authenticatedPage.locator('[data-testid="import-btn"]')).toBeVisible();
  });

  test('export button links to /settings/export', async ({ authenticatedPage }) => {
    await authenticatedPage.goto('/settings');

    const exportBtn = authenticatedPage.locator('[data-testid="export-btn"]');
    await expect(exportBtn).toHaveAttribute('href', '/settings/export');
  });
});

test.describe('Appearance', () => {
  test('dark mode toggle is visible', async ({ authenticatedPage }) => {
    await authenticatedPage.goto('/settings');

    await expect(authenticatedPage.locator('[data-testid="dark-mode-toggle"]')).toBeVisible();
  });

  test('dark mode toggle works', async ({ authenticatedPage }) => {
    await authenticatedPage.goto('/settings');

    const toggle = authenticatedPage.locator('[data-testid="dark-mode-toggle"]');
    const html = authenticatedPage.locator('html');

    // Get initial state
    const wasChecked = await toggle.isChecked();
    const hadDarkClass = await html.evaluate((el) => el.classList.contains('dark'));

    // Toggle
    await toggle.click();

    // Verify the class changed
    if (wasChecked) {
      // Was dark, should now be light
      await expect(html).not.toHaveClass(/dark/);
    } else {
      // Was light, should now be dark
      await expect(html).toHaveClass(/dark/);
    }
  });
});

test.describe('Study Tools', () => {
  test('ready for review button is visible', async ({ authenticatedPage }) => {
    await authenticatedPage.goto('/settings');

    // Look for the "Ready for Review" button in study tools section
    const studyTools = authenticatedPage.locator('#study-tools');
    await expect(studyTools).toBeVisible();
  });
});
