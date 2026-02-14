import { test, expect } from '../fixtures/auth';
import { Page } from '@playwright/test';

/**
 * Vocabulary Library Search E2E Tests
 *
 * Uses test_vocabulary_pack fixture with Korean vocabulary entries
 * including romanization, translation, notes, usages, and examples.
 */

/** Skip test if vocabulary pack is not enabled or accessible */
async function ensureVocabularyAvailable(page: Page): Promise<void> {
  const searchInput = page.locator('#vocab-search-input');
  const notEnabled = page.locator('text=Vocabulary Pack Not Enabled');
  const hasSearch = await searchInput.isVisible().catch(() => false);
  const isDisabled = await notEnabled.isVisible().catch(() => false);
  if (!hasSearch || isDisabled) {
    test.skip(true, 'Vocabulary pack not enabled or accessible');
  }
}

test.describe('Vocabulary Library Search', () => {
  // Run tests serially to ensure setup completes before other tests
  test.describe.configure({ mode: 'serial' });
  // Setup: Enable the test vocabulary pack before running tests
  test.describe('Pack Setup', () => {
    test('enable test vocabulary pack for all users', async ({ adminPage }) => {
      await adminPage.goto('/settings');

      const testPackCard = adminPage.locator('#pack-card-test_vocabulary_pack');

      // Check if pack exists
      const packExists = await testPackCard.isVisible().catch(() => false);
      test.skip(!packExists, 'test_vocabulary_pack not found - check global-setup.ts');

      // Check if already enabled (green background)
      const isEnabled = await testPackCard.evaluate((el) =>
        el.classList.contains('bg-green-50') || el.className.includes('bg-green-')
      );

      if (!isEnabled) {
        const enableBtn = testPackCard.locator('button:has-text("Enable")');
        await enableBtn.click();

        // Wait for HTMX to swap the element with green background
        // Re-locate since outerHTML replaces the element
        await expect(adminPage.locator('#pack-card-test_vocabulary_pack')).toHaveClass(/bg-green/, { timeout: 15000 });
      } else {
        await expect(testPackCard).toHaveClass(/bg-green/);
      }

      // Accept the confirmation dialog when Make Public is clicked
      adminPage.on('dialog', async dialog => {
        await dialog.accept();
      });

      // Expand "Manage access" to show permissions
      const manageAccessSummary = testPackCard.locator('summary:has-text("Manage access")');
      if (await manageAccessSummary.isVisible()) {
        await manageAccessSummary.click();
        // Wait for the details element to expand
        await expect(adminPage.locator('#pack-permissions-test_vocabulary_pack')).toBeVisible();
      }

      // Make the pack public so it shows up for all users
      const permissionsSection = adminPage.locator('#pack-permissions-test_vocabulary_pack');
      const makePublicBtn = permissionsSection.locator('button:has-text("Make Public")');

      if (await makePublicBtn.isVisible()) {
        await makePublicBtn.click();
        // Wait for the button to disappear after HTMX swap
        await expect(makePublicBtn).not.toBeVisible();
      }
    });
  });

  test.describe('Search UI', () => {
    test('should display search input on vocabulary page', async ({ authenticatedPage }) => {
      await authenticatedPage.goto('/library/vocabulary');
      await ensureVocabularyAvailable(authenticatedPage);

      const searchInput = authenticatedPage.locator('#vocab-search-input');
      await expect(searchInput).toBeVisible();
      await expect(searchInput).toHaveAttribute('placeholder', /Search vocabulary/);
    });

    test('should show word count', async ({ authenticatedPage }) => {
      await authenticatedPage.goto('/library/vocabulary');
      await ensureVocabularyAvailable(authenticatedPage);

      const resultCount = authenticatedPage.locator('#vocab-result-count');
      await expect(resultCount).toBeVisible();
      await expect(resultCount).toContainText('words');
    });

    test('should focus search input when pressing /', async ({ authenticatedPage }) => {
      await authenticatedPage.goto('/library/vocabulary');
      await ensureVocabularyAvailable(authenticatedPage);

      const searchInput = authenticatedPage.locator('#vocab-search-input');

      // Wait for page to be fully loaded and interactive
      await expect(authenticatedPage.locator('#vocab-result-count')).toBeVisible();

      // Ensure document has focus (click on body first)
      await authenticatedPage.locator('body').click();

      await authenticatedPage.keyboard.press('/');
      // backToSearch() has a 300ms setTimeout before focusing, allow extra time
      await expect(searchInput).toBeFocused({ timeout: 2000 });
    });
  });

  test.describe('Search Functionality', () => {
    test('should show results dropdown when typing', async ({ authenticatedPage }) => {
      await authenticatedPage.goto('/library/vocabulary');
      await ensureVocabularyAvailable(authenticatedPage);

      const searchInput = authenticatedPage.locator('#vocab-search-input');

      // Wait for word count to appear (indicates vocabulary JS is initialized)
      await expect(authenticatedPage.locator('#vocab-result-count')).toContainText('words');

      const resultsDropdown = authenticatedPage.locator('#vocab-search-results');

      await expect(resultsDropdown).toBeHidden();
      await searchInput.click();
      await searchInput.fill('ko');
      // Wait for debounce (150ms) + rendering time
      await expect(resultsDropdown).toBeVisible({ timeout: 5000 });
    });

    test('should show clickable results with Korean term and romanization', async ({ authenticatedPage }) => {
      await authenticatedPage.goto('/library/vocabulary');
      await ensureVocabularyAvailable(authenticatedPage);

      const searchInput = authenticatedPage.locator('#vocab-search-input');
      await searchInput.fill('han');

      const resultsDropdown = authenticatedPage.locator('#vocab-search-results');
      await expect(resultsDropdown).toBeVisible({ timeout: 5000 });

      const results = authenticatedPage.locator('.vocab-result');
      const count = await results.count();

      if (count > 0) {
        const firstResult = results.first();
        await expect(firstResult).toBeVisible();
      }
    });

    test('should update match count when searching', async ({ authenticatedPage }) => {
      await authenticatedPage.goto('/library/vocabulary');
      await ensureVocabularyAvailable(authenticatedPage);

      const searchInput = authenticatedPage.locator('#vocab-search-input');
      const resultCount = authenticatedPage.locator('#vocab-result-count');

      await searchInput.fill('han');
      // Wait for debounce to update the result count
      await expect(resultCount).toContainText(/matches|words/);
    });

    test('should show no results message for non-matching query', async ({ authenticatedPage }) => {
      await authenticatedPage.goto('/library/vocabulary');
      await ensureVocabularyAvailable(authenticatedPage);

      const searchInput = authenticatedPage.locator('#vocab-search-input');
      await searchInput.fill('xyznonexistent123');

      const resultsDropdown = authenticatedPage.locator('#vocab-search-results');
      await expect(resultsDropdown).toBeVisible({ timeout: 5000 });
      await expect(resultsDropdown).toContainText('No vocabulary matches');
    });

    test('should clear search and hide results when pressing Escape', async ({ authenticatedPage }) => {
      await authenticatedPage.goto('/library/vocabulary');
      await ensureVocabularyAvailable(authenticatedPage);

      const searchInput = authenticatedPage.locator('#vocab-search-input');
      const resultsDropdown = authenticatedPage.locator('#vocab-search-results');

      await searchInput.fill('han');
      await expect(resultsDropdown).toBeVisible({ timeout: 5000 });

      await searchInput.press('Escape');

      await expect(resultsDropdown).toBeHidden();
      await expect(searchInput).toHaveValue('');
    });

    test('should show clear button when input has text', async ({ authenticatedPage }) => {
      await authenticatedPage.goto('/library/vocabulary');
      await ensureVocabularyAvailable(authenticatedPage);

      const searchInput = authenticatedPage.locator('#vocab-search-input');
      const clearButton = authenticatedPage.locator('#vocab-search-clear');

      await expect(clearButton).toBeHidden();
      await searchInput.fill('test');
      await expect(clearButton).toBeVisible();
    });

    test('should clear search when clicking clear button', async ({ authenticatedPage }) => {
      await authenticatedPage.goto('/library/vocabulary');
      await ensureVocabularyAvailable(authenticatedPage);

      const searchInput = authenticatedPage.locator('#vocab-search-input');
      const clearButton = authenticatedPage.locator('#vocab-search-clear');

      await searchInput.fill('test');
      await expect(clearButton).toBeVisible();

      await clearButton.click();

      await expect(searchInput).toHaveValue('');
      await expect(clearButton).toBeHidden();
    });
  });

  test.describe('Result Navigation', () => {
    test('should navigate to entry when clicking result', async ({ authenticatedPage }) => {
      await authenticatedPage.goto('/library/vocabulary');
      await ensureVocabularyAvailable(authenticatedPage);

      const searchInput = authenticatedPage.locator('#vocab-search-input');
      await searchInput.fill('han');

      const resultsDropdown = authenticatedPage.locator('#vocab-search-results');
      await expect(resultsDropdown).toBeVisible({ timeout: 5000 });

      const firstResult = authenticatedPage.locator('.vocab-result').first();
      const vocabId = await firstResult.getAttribute('data-vocab-id');

      if (vocabId) {
        await firstResult.click();

        await expect(resultsDropdown).toBeHidden();

        // The entry in the main list should be visible and open
        const entry = authenticatedPage.locator(`details[data-vocab-id="${vocabId}"]`);
        await expect(entry).toBeVisible();
        await expect(entry).toHaveAttribute('open', '');
      }
    });

    test('should support keyboard navigation in results', async ({ authenticatedPage }) => {
      await authenticatedPage.goto('/library/vocabulary');
      await ensureVocabularyAvailable(authenticatedPage);

      const searchInput = authenticatedPage.locator('#vocab-search-input');
      await searchInput.fill('han');

      // Wait for results to appear before navigating
      const resultsDropdown = authenticatedPage.locator('#vocab-search-results');
      await expect(resultsDropdown).toBeVisible({ timeout: 5000 });

      await searchInput.press('ArrowDown');

      const firstResult = authenticatedPage.locator('.vocab-result').first();
      await expect(firstResult).toBeFocused();

      await firstResult.press('ArrowUp');
      await expect(searchInput).toBeFocused();
    });

    test('should show back button after navigating to result', async ({ authenticatedPage }) => {
      await authenticatedPage.goto('/library/vocabulary');
      await ensureVocabularyAvailable(authenticatedPage);

      const searchInput = authenticatedPage.locator('#vocab-search-input');
      const backButton = authenticatedPage.locator('#vocab-back-to-search');

      // Back button should be hidden initially
      await expect(backButton).toBeHidden();

      // Search and click a result
      await searchInput.fill('han');
      const resultsDropdown = authenticatedPage.locator('#vocab-search-results');
      await expect(resultsDropdown).toBeVisible({ timeout: 5000 });

      const firstResult = authenticatedPage.locator('.vocab-result').first();
      await firstResult.click();

      // Back button should now be visible
      await expect(backButton).toBeVisible();
    });

    test('should scroll to search and focus input when clicking back button', async ({ authenticatedPage }) => {
      await authenticatedPage.goto('/library/vocabulary');
      await ensureVocabularyAvailable(authenticatedPage);

      const searchInput = authenticatedPage.locator('#vocab-search-input');
      const backButton = authenticatedPage.locator('#vocab-back-to-search');

      // Navigate to a result first
      await searchInput.fill('han');
      const resultsDropdown = authenticatedPage.locator('#vocab-search-results');
      await expect(resultsDropdown).toBeVisible({ timeout: 5000 });

      const firstResult = authenticatedPage.locator('.vocab-result').first();
      await firstResult.click();
      await expect(backButton).toBeVisible();

      // Click the back button
      await backButton.click();

      // Search input should be focused (scroll + 300ms focus delay)
      await expect(searchInput).toBeFocused({ timeout: 2000 });

      // Back button should be hidden
      await expect(backButton).toBeHidden();
    });

    test('should hide back button when clicking elsewhere on page', async ({ authenticatedPage }) => {
      await authenticatedPage.goto('/library/vocabulary');
      await ensureVocabularyAvailable(authenticatedPage);

      const searchInput = authenticatedPage.locator('#vocab-search-input');
      const backButton = authenticatedPage.locator('#vocab-back-to-search');

      // Navigate to a result first
      await searchInput.fill('han');
      const resultsDropdown = authenticatedPage.locator('#vocab-search-results');
      await expect(resultsDropdown).toBeVisible({ timeout: 5000 });

      const firstResult = authenticatedPage.locator('.vocab-result').first();
      await firstResult.click();
      await expect(backButton).toBeVisible();

      // Click somewhere else on the page (the main content area)
      await authenticatedPage.locator('main').click({ position: { x: 100, y: 300 } });

      // Back button should be hidden
      await expect(backButton).toBeHidden();
    });
  });

  test.describe('Fuzzy Matching', () => {
    test('should match by romanization', async ({ authenticatedPage }) => {
      await authenticatedPage.goto('/library/vocabulary');
      await ensureVocabularyAvailable(authenticatedPage);

      const searchInput = authenticatedPage.locator('#vocab-search-input');
      await searchInput.fill('han-guk');

      const resultsDropdown = authenticatedPage.locator('#vocab-search-results');
      await expect(resultsDropdown).toBeVisible({ timeout: 5000 });

      const results = authenticatedPage.locator('.vocab-result');
      const count = await results.count();
      expect(count).toBeGreaterThan(0);
    });

    test('should match by translation/meaning', async ({ authenticatedPage }) => {
      await authenticatedPage.goto('/library/vocabulary');
      await ensureVocabularyAvailable(authenticatedPage);

      const searchInput = authenticatedPage.locator('#vocab-search-input');
      await searchInput.fill('Korea');

      const resultsDropdown = authenticatedPage.locator('#vocab-search-results');
      await expect(resultsDropdown).toBeVisible({ timeout: 5000 });

      const results = authenticatedPage.locator('.vocab-result');
      const count = await results.count();
      expect(count).toBeGreaterThan(0);
    });

    test('should handle typos with fuzzy matching', async ({ authenticatedPage }) => {
      await authenticatedPage.goto('/library/vocabulary');
      await ensureVocabularyAvailable(authenticatedPage);

      const searchInput = authenticatedPage.locator('#vocab-search-input');
      await searchInput.fill('hanguc'); // typo for han-guk

      const resultsDropdown = authenticatedPage.locator('#vocab-search-results');
      await expect(resultsDropdown).toBeVisible({ timeout: 5000 });
    });
  });
});
