/**
 * E2E tests for offline study mode.
 *
 * Tests the full offline study flow including:
 * - Enabling offline mode in settings
 * - Downloading a session
 * - Studying offline
 * - Syncing when back online
 */
import { test, expect } from '@playwright/test';

// These tests need their own project with appropriate setup
// Add to playwright.config.ts:
// { name: 'offline-tests', testMatch: /offline-study\.spec\.ts/, use: { ...devices['Desktop Chrome'] } }

test.describe('Offline Study Mode', () => {
  test.beforeEach(async ({ page, context }) => {
    // Login as test user
    await page.goto('/login');
    await page.fill('input[name="username"]', 'testuser');
    await page.fill('input[name="password"]', 'testpassword');
    await page.click('button[type="submit"]');
    await expect(page).toHaveURL('/');
  });

  test('can enable offline mode in settings', async ({ page }) => {
    await page.goto('/settings');

    // Find offline mode section
    const offlineSection = page.locator('#offline-mode');
    await expect(offlineSection).toBeVisible();

    // Check browser support shows as supported
    const status = offlineSection.locator('#offline-status');
    await expect(status).toContainText('All features supported');

    // Enable offline mode
    const toggle = page.locator('#offlineModeToggle');
    if (!(await toggle.isChecked())) {
      await toggle.click();
    }

    // Save settings
    await offlineSection.locator('button[type="submit"]').click();

    // Verify options appear
    const downloadSection = page.locator('#offline-download');
    await expect(downloadSection).toBeVisible();
  });

  test('can download offline session', async ({ page }) => {
    // First enable offline mode
    await page.goto('/settings');
    const toggle = page.locator('#offlineModeToggle');
    if (!(await toggle.isChecked())) {
      await toggle.click();
      await page.locator('#offline-mode button[type="submit"]').click();
    }

    // Click download
    const downloadBtn = page.locator('#download-session-btn');
    await downloadBtn.click();

    // Wait for download to complete
    const status = page.locator('#download-status');
    await expect(status).toContainText('Downloaded', { timeout: 10000 });
    await expect(status).toContainText('cards');

    // Verify IndexedDB has data
    const hasSession = await page.evaluate(async () => {
      return new Promise((resolve) => {
        const request = indexedDB.open('kr-offline-study', 1);
        request.onsuccess = () => {
          const db = request.result;
          const tx = db.transaction('sessions', 'readonly');
          const store = tx.objectStore('sessions');
          const getAll = store.getAll();
          getAll.onsuccess = () => {
            resolve(getAll.result.length > 0);
          };
          getAll.onerror = () => resolve(false);
        };
        request.onerror = () => resolve(false);
      });
    });

    expect(hasSession).toBe(true);
  });

  test('offline study page loads with session', async ({ page, context }) => {
    // Download session first
    await page.goto('/settings');
    const toggle = page.locator('#offlineModeToggle');
    if (!(await toggle.isChecked())) {
      await toggle.click();
      await page.locator('#offline-mode button[type="submit"]').click();
    }
    await page.locator('#download-session-btn').click();
    await expect(page.locator('#download-status')).toContainText('Downloaded', { timeout: 10000 });

    // Go to offline study page directly
    await page.goto('/offline-study');

    // Should show session ready state
    await expect(page.locator('#session-ready-state')).toBeVisible({ timeout: 5000 });
    await expect(page.locator('#session-card-count')).not.toHaveText('0');
  });

  test('can complete study cards offline', async ({ page, context }) => {
    // Download session
    await page.goto('/settings');
    const toggle = page.locator('#offlineModeToggle');
    if (!(await toggle.isChecked())) {
      await toggle.click();
      await page.locator('#offline-mode button[type="submit"]').click();
    }
    await page.locator('#download-session-btn').click();
    await expect(page.locator('#download-status')).toContainText('Downloaded', { timeout: 10000 });

    // Go to offline study
    await page.goto('/offline-study');
    await expect(page.locator('#session-ready-state')).toBeVisible({ timeout: 5000 });

    // Start studying
    await page.locator('#start-study-btn').click();

    // Should show study active state
    await expect(page.locator('#study-active-state')).toBeVisible();
    await expect(page.locator('#card-container')).toBeVisible();

    // Wait for card to render
    await expect(page.locator('.offline-card')).toBeVisible({ timeout: 5000 });

    // Answer a card (click first choice if multiple choice)
    const choiceBtn = page.locator('.choice-btn').first();
    if (await choiceBtn.isVisible()) {
      await choiceBtn.click();
    } else {
      // Text input mode
      const input = page.locator('.answer-input');
      await input.fill('test answer');
      await page.locator('.submit-btn').click();
    }

    // Should show result
    await expect(page.locator('.result-section')).toBeVisible({ timeout: 5000 });

    // Click next
    await page.locator('.next-btn').click();

    // Verify response was stored
    const responseCount = await page.evaluate(async () => {
      return new Promise((resolve) => {
        const request = indexedDB.open('kr-offline-study', 1);
        request.onsuccess = () => {
          const db = request.result;
          const tx = db.transaction('responses', 'readonly');
          const store = tx.objectStore('responses');
          const count = store.count();
          count.onsuccess = () => resolve(count.result);
          count.onerror = () => resolve(0);
        };
        request.onerror = () => resolve(0);
      });
    });

    expect(responseCount).toBeGreaterThan(0);
  });

  test('sync notification appears when coming online', async ({ page, context }) => {
    // Download and do some offline study
    await page.goto('/settings');
    const toggle = page.locator('#offlineModeToggle');
    if (!(await toggle.isChecked())) {
      await toggle.click();
      await page.locator('#offline-mode button[type="submit"]').click();
    }
    await page.locator('#download-session-btn').click();
    await expect(page.locator('#download-status')).toContainText('Downloaded', { timeout: 10000 });

    // Go to offline study and answer one card
    await page.goto('/offline-study');
    await expect(page.locator('#session-ready-state')).toBeVisible({ timeout: 5000 });
    await page.locator('#start-study-btn').click();
    await expect(page.locator('.offline-card')).toBeVisible({ timeout: 5000 });

    const choiceBtn = page.locator('.choice-btn').first();
    if (await choiceBtn.isVisible()) {
      await choiceBtn.click();
    } else {
      await page.locator('.answer-input').fill('test');
      await page.locator('.submit-btn').click();
    }

    // Go back to home (this triggers sync check)
    await page.goto('/');

    // Wait for sync notification (the OfflineSync module checks on page load)
    // Note: This may require the page to have pending responses
    const notification = page.locator('#offline-sync-notification');

    // Give it time to appear (2 second delay in the module)
    await expect(notification).toBeVisible({ timeout: 5000 });
    await expect(notification).toContainText('Offline Progress Ready');
  });

  test('can sync offline progress', async ({ page }) => {
    // Setup: download session and create some responses
    await page.goto('/settings');
    const toggle = page.locator('#offlineModeToggle');
    if (!(await toggle.isChecked())) {
      await toggle.click();
      await page.locator('#offline-mode button[type="submit"]').click();
    }
    await page.locator('#download-session-btn').click();
    await expect(page.locator('#download-status')).toContainText('Downloaded', { timeout: 10000 });

    // Do some offline study
    await page.goto('/offline-study');
    await expect(page.locator('#session-ready-state')).toBeVisible({ timeout: 5000 });
    await page.locator('#start-study-btn').click();
    await expect(page.locator('.offline-card')).toBeVisible({ timeout: 5000 });

    const choiceBtn = page.locator('.choice-btn').first();
    if (await choiceBtn.isVisible()) {
      await choiceBtn.click();
    } else {
      await page.locator('.answer-input').fill('test');
      await page.locator('.submit-btn').click();
    }

    // Trigger sync check
    await page.goto('/');
    const notification = page.locator('#offline-sync-notification');
    await expect(notification).toBeVisible({ timeout: 5000 });

    // Click sync
    await page.locator('#sync-now-btn').click();

    // Wait for success
    await expect(page.locator('#sync-result')).toContainText('Successfully synced', { timeout: 10000 });

    // Verify IndexedDB cleared
    const hasResponses = await page.evaluate(async () => {
      return new Promise((resolve) => {
        const request = indexedDB.open('kr-offline-study', 1);
        request.onsuccess = () => {
          const db = request.result;
          const tx = db.transaction('responses', 'readonly');
          const store = tx.objectStore('responses');
          const count = store.count();
          count.onsuccess = () => resolve(count.result > 0);
          count.onerror = () => resolve(false);
        };
        request.onerror = () => resolve(false);
      });
    });

    expect(hasResponses).toBe(false);
  });
});

test.describe('Offline Study - No Session', () => {
  test.beforeEach(async ({ page }) => {
    await page.goto('/login');
    await page.fill('input[name="username"]', 'testuser');
    await page.fill('input[name="password"]', 'testpassword');
    await page.click('button[type="submit"]');
  });

  test('shows no session message when none downloaded', async ({ page }) => {
    // Clear any existing IndexedDB data
    await page.evaluate(async () => {
      const dbs = await indexedDB.databases();
      for (const db of dbs) {
        if (db.name === 'kr-offline-study') {
          indexedDB.deleteDatabase(db.name!);
        }
      }
    });

    await page.goto('/offline-study');

    // Should show no session state
    await expect(page.locator('#no-session-state')).toBeVisible({ timeout: 5000 });
    await expect(page.locator('#no-session-state')).toContainText('No Offline Session');
  });
});
