import { test, expect } from '../fixtures/auth';

test.describe('Learning Settings', () => {
  test('can toggle unlock all tiers', async ({ authenticatedPage }) => {
    await authenticatedPage.goto('/settings');

    const toggle = authenticatedPage.locator('[data-testid="all-tiers-toggle"]');
    await expect(toggle).toBeVisible();

    const wasChecked = await toggle.isChecked();

    await toggle.click({ force: true });

    const isNowChecked = await toggle.isChecked();
    expect(isNowChecked).toBe(!wasChecked);

    await Promise.all([
      authenticatedPage.waitForResponse(resp => resp.url().includes('/settings') && resp.request().method() === 'POST'),
      authenticatedPage.locator('[data-testid="settings-submit"]').click()
    ]);

    await authenticatedPage.waitForLoadState('domcontentloaded');
    const newToggle = authenticatedPage.locator('[data-testid="all-tiers-toggle"]');
    const finalState = await newToggle.isChecked();
    expect(finalState).toBe(!wasChecked);
  });

  test('tier options show when unlock all tiers is enabled', async ({ authenticatedPage }) => {
    await authenticatedPage.goto('/settings');

    const toggle = authenticatedPage.locator('[data-testid="all-tiers-toggle"]');
    const tierOptions = authenticatedPage.locator('#tierOptions');

    if (!(await toggle.isChecked())) {
      await toggle.click();
    }

    await expect(tierOptions).toBeVisible();

    await expect(tierOptions.locator('input[name="tier_1"]')).toBeVisible();
    await expect(tierOptions.locator('input[name="tier_2"]')).toBeVisible();
  });

  test('can change retention target', async ({ authenticatedPage }) => {
    await authenticatedPage.goto('/settings');

    const retentionSelect = authenticatedPage.locator('select[name="desired_retention"]');
    await expect(retentionSelect).toBeVisible();

    await retentionSelect.selectOption('85');

    await Promise.all([
      authenticatedPage.waitForResponse(resp => resp.url().includes('/settings') && resp.request().method() === 'POST'),
      authenticatedPage.locator('[data-testid="settings-submit"]').click()
    ]);

    await authenticatedPage.waitForLoadState('domcontentloaded');
    const newSelect = authenticatedPage.locator('select[name="desired_retention"]');
    await expect(newSelect).toHaveValue('85');
  });

  test('can toggle focus mode', async ({ authenticatedPage }) => {
    await authenticatedPage.goto('/settings');

    const focusCheckbox = authenticatedPage.locator('input[name="focus_mode"]');
    await expect(focusCheckbox).toBeVisible();

    const wasChecked = await focusCheckbox.isChecked();
    await focusCheckbox.click();

    await Promise.all([
      authenticatedPage.waitForResponse(resp => resp.url().includes('/settings') && resp.request().method() === 'POST'),
      authenticatedPage.locator('[data-testid="settings-submit"]').click()
    ]);

    await authenticatedPage.waitForLoadState('domcontentloaded');
    const newCheckbox = authenticatedPage.locator('input[name="focus_mode"]');
    await expect(newCheckbox).toBeChecked({ checked: !wasChecked });
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

    const wasChecked = await toggle.isChecked();

    await toggle.click();

    if (wasChecked) {
      await expect(html).not.toHaveClass(/dark/);
    } else {
      await expect(html).toHaveClass(/dark/);
    }
  });
});

test.describe('Study Tools', () => {
  test('ready for review button is visible', async ({ authenticatedPage }) => {
    await authenticatedPage.goto('/settings');

    const studyTools = authenticatedPage.locator('#study-tools');
    await expect(studyTools).toBeVisible();

    const readyButton = studyTools.locator('button:has-text("Ready"), a:has-text("Ready")');
    await expect(readyButton.first()).toBeVisible();
  });
});
