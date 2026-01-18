/**
 * E2E tests for offline study mode.
 *
 * Tests the full offline study flow including:
 * - Enabling offline mode in settings
 * - Downloading a session
 * - Studying offline
 * - Syncing when back online
 */
import { test, expect, setupScenario } from '../fixtures/auth';

test.describe('Offline Study Mode', () => {
  test('can enable offline mode in settings', async ({ authenticatedPage }) => {
    await authenticatedPage.goto('/settings');

    // Find offline mode section
    const offlineSection = authenticatedPage.locator('#offline-mode');
    await expect(offlineSection).toBeVisible();

    // Check browser support shows as supported
    const status = offlineSection.locator('#offline-status');
    await expect(status).toContainText('All features supported');

    // Enable offline mode
    const toggle = authenticatedPage.locator('#offlineModeToggle');
    if (!(await toggle.isChecked())) {
      await toggle.click();
    }

    // Save settings
    await offlineSection.locator('button[type="submit"]').click();

    // Verify options appear
    const downloadSection = authenticatedPage.locator('#offline-download');
    await expect(downloadSection).toBeVisible();
  });

  test('can download offline session', async ({ authenticatedPage, testUser }) => {
    // Set up scenario with due cards
    setupScenario(testUser.username, 'tier1_new', testUser.dataDir);

    // First enable offline mode
    await authenticatedPage.goto('/settings');
    const toggle = authenticatedPage.locator('#offlineModeToggle');
    if (!(await toggle.isChecked())) {
      await toggle.click();
      await authenticatedPage.locator('#offline-mode button[type="submit"]').click();
    }

    // Click download
    const downloadBtn = authenticatedPage.locator('#download-session-btn');
    await downloadBtn.click();

    // Wait for download to complete
    const status = authenticatedPage.locator('#download-status');
    await expect(status).toContainText('Downloaded', { timeout: 10000 });
    await expect(status).toContainText('cards');

    // Verify IndexedDB has data
    const hasSession = await authenticatedPage.evaluate(async () => {
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

  test('offline study page loads with session', async ({ authenticatedPage, testUser }) => {
    // Set up scenario with due cards
    setupScenario(testUser.username, 'tier1_new', testUser.dataDir);

    // Download session first
    await authenticatedPage.goto('/settings');
    const toggle = authenticatedPage.locator('#offlineModeToggle');
    if (!(await toggle.isChecked())) {
      await toggle.click();
      await authenticatedPage.locator('#offline-mode button[type="submit"]').click();
    }
    await authenticatedPage.locator('#download-session-btn').click();
    await expect(authenticatedPage.locator('#download-status')).toContainText('Downloaded', { timeout: 10000 });

    // Go to offline study page directly
    await authenticatedPage.goto('/offline-study');

    // Should show session ready state
    await expect(authenticatedPage.locator('#session-ready-state')).toBeVisible({ timeout: 5000 });
    await expect(authenticatedPage.locator('#session-card-count')).not.toHaveText('0');
  });

  test('can complete study cards offline', async ({ authenticatedPage, testUser }) => {
    // Set up scenario with due cards
    setupScenario(testUser.username, 'tier1_new', testUser.dataDir);

    // Download session
    await authenticatedPage.goto('/settings');
    const toggle = authenticatedPage.locator('#offlineModeToggle');
    if (!(await toggle.isChecked())) {
      await toggle.click();
      await authenticatedPage.locator('#offline-mode button[type="submit"]').click();
    }
    await authenticatedPage.locator('#download-session-btn').click();
    await expect(authenticatedPage.locator('#download-status')).toContainText('Downloaded', { timeout: 10000 });

    // Go to offline study
    await authenticatedPage.goto('/offline-study');
    await expect(authenticatedPage.locator('#session-ready-state')).toBeVisible({ timeout: 5000 });

    // Start studying
    await authenticatedPage.locator('#start-study-btn').click();

    // Should show study active state
    await expect(authenticatedPage.locator('#study-active-state')).toBeVisible();
    await expect(authenticatedPage.locator('#card-container')).toBeVisible();

    // Wait for card to render
    await expect(authenticatedPage.locator('.offline-card')).toBeVisible({ timeout: 5000 });

    // Answer a card (click first choice if multiple choice)
    const choiceBtn = authenticatedPage.locator('.choice-btn').first();
    if (await choiceBtn.isVisible()) {
      await choiceBtn.click();
    } else {
      // Text input mode
      const input = authenticatedPage.locator('.answer-input');
      await input.fill('test answer');
      await authenticatedPage.locator('.submit-btn').click();
    }

    // Should show result
    await expect(authenticatedPage.locator('.result-section')).toBeVisible({ timeout: 5000 });

    // Click next
    await authenticatedPage.locator('.continue-btn').click();

    // Verify response was stored
    const responseCount = await authenticatedPage.evaluate(async () => {
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

  test('auto-sync triggers when navigating with pending reviews', async ({ authenticatedPage, testUser }) => {
    // Set up scenario with due cards
    setupScenario(testUser.username, 'tier1_new', testUser.dataDir);

    // Download and do some offline study
    await authenticatedPage.goto('/settings');
    const toggle = authenticatedPage.locator('#offlineModeToggle');
    if (!(await toggle.isChecked())) {
      await toggle.click();
      await authenticatedPage.locator('#offline-mode button[type="submit"]').click();
    }
    await authenticatedPage.locator('#download-session-btn').click();
    await expect(authenticatedPage.locator('#download-status')).toContainText('Downloaded', { timeout: 10000 });

    // Go to offline study and answer one card
    await authenticatedPage.goto('/offline-study');
    await expect(authenticatedPage.locator('#session-ready-state')).toBeVisible({ timeout: 5000 });
    await authenticatedPage.locator('#start-study-btn').click();
    await expect(authenticatedPage.locator('.offline-card')).toBeVisible({ timeout: 5000 });

    const choiceBtn = authenticatedPage.locator('.choice-btn').first();
    if (await choiceBtn.isVisible()) {
      await choiceBtn.click();
    } else {
      await authenticatedPage.locator('.answer-input').fill('test');
      await authenticatedPage.locator('.submit-btn').click();
    }

    // Verify we have pending responses before navigating
    const pendingBefore = await authenticatedPage.evaluate(async () => {
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
    expect(pendingBefore).toBeGreaterThan(0);

    // Go back to home - this triggers sync prompt after stability delay
    await authenticatedPage.goto('/');

    // Set short stability delay for testing
    await authenticatedPage.evaluate(() => {
      window.OfflineSyncTestAPI.setStabilityDelay(100);
    });

    // Simulate coming online to trigger stability timer
    await authenticatedPage.evaluate(() => {
      window.OfflineSyncTestAPI.simulateOnline();
    });

    // Wait for sync prompt modal to appear
    const syncPrompt = authenticatedPage.locator('#sync-prompt-modal');
    await expect(syncPrompt).toBeVisible({ timeout: 5000 });

    // Verify pending count is shown
    const countEl = authenticatedPage.locator('#sync-prompt-count');
    await expect(countEl).toBeVisible();

    // Click sync now
    await authenticatedPage.click('#sync-now-btn');

    // Modal should close and sync notification should appear briefly
    await expect(syncPrompt).toBeHidden({ timeout: 10000 });
  });

  test('auto-sync clears pending reviews from IndexedDB', async ({ authenticatedPage, testUser }) => {
    // Set up scenario with due cards
    setupScenario(testUser.username, 'tier1_new', testUser.dataDir);

    // Setup: download session and create some responses
    await authenticatedPage.goto('/settings');
    const toggle = authenticatedPage.locator('#offlineModeToggle');
    if (!(await toggle.isChecked())) {
      await toggle.click();
      await authenticatedPage.locator('#offline-mode button[type="submit"]').click();
    }
    await authenticatedPage.locator('#download-session-btn').click();
    await expect(authenticatedPage.locator('#download-status')).toContainText('Downloaded', { timeout: 10000 });

    // Do some offline study
    await authenticatedPage.goto('/offline-study');
    await expect(authenticatedPage.locator('#session-ready-state')).toBeVisible({ timeout: 5000 });
    await authenticatedPage.locator('#start-study-btn').click();
    await expect(authenticatedPage.locator('.offline-card')).toBeVisible({ timeout: 5000 });

    const choiceBtn = authenticatedPage.locator('.choice-btn').first();
    if (await choiceBtn.isVisible()) {
      await choiceBtn.click();
    } else {
      await authenticatedPage.locator('.answer-input').fill('test');
      await authenticatedPage.locator('.submit-btn').click();
    }

    // Navigate to trigger sync flow
    await authenticatedPage.goto('/');

    // Set short stability delay for testing
    await authenticatedPage.evaluate(() => {
      window.OfflineSyncTestAPI.setStabilityDelay(100);
    });

    // Simulate coming online to trigger stability timer
    await authenticatedPage.evaluate(() => {
      window.OfflineSyncTestAPI.simulateOnline();
    });

    // Wait for sync prompt modal to appear
    const syncPrompt = authenticatedPage.locator('#sync-prompt-modal');
    await expect(syncPrompt).toBeVisible({ timeout: 5000 });

    // Click sync now to trigger the sync
    await authenticatedPage.click('#sync-now-btn');

    // Wait for modal to close (sync completed)
    await expect(syncPrompt).toBeHidden({ timeout: 10000 });

    // Poll for IndexedDB to be cleared (webkit is slower)
    let hasResponses = true;
    for (let i = 0; i < 10; i++) {
      await authenticatedPage.waitForTimeout(200);
      hasResponses = await authenticatedPage.evaluate(async () => {
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
      if (!hasResponses) break;
    }

    expect(hasResponses).toBe(false);
  });
});

test.describe('Offline Study - No Session', () => {
  test('shows no session message when none downloaded', async ({ authenticatedPage }) => {
    // Clear any existing IndexedDB data
    await authenticatedPage.evaluate(async () => {
      const dbs = await indexedDB.databases();
      for (const db of dbs) {
        if (db.name === 'kr-offline-study') {
          indexedDB.deleteDatabase(db.name!);
        }
      }
    });

    await authenticatedPage.goto('/offline-study');

    // Should show no session state
    await expect(authenticatedPage.locator('#no-session-state')).toBeVisible({ timeout: 5000 });
    await expect(authenticatedPage.locator('#no-session-state')).toContainText('No Offline Session');
  });
});
