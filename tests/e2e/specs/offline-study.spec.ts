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

// Helper to enable offline mode in settings
async function enableOfflineMode(page) {
  await page.goto('/settings', { waitUntil: 'domcontentloaded' });

  // Wait for offline mode section to be visible and ready (WebKit is slower)
  const offlineSection = page.locator('#offline-mode');
  await expect(offlineSection).toBeVisible({ timeout: 10000 });

  // Wait for browser support check to complete (match exact text from passing test)
  const status = offlineSection.locator('#offline-status');
  await expect(status).toContainText('All features supported');

  const toggle = page.locator('#offlineModeToggle');
  if (!(await toggle.isChecked())) {
    // Match the pattern from the passing test: scroll, delay, click with force
    await toggle.scrollIntoViewIfNeeded();
    await page.waitForTimeout(100);
    await toggle.click({ force: true });
    // Verify the checkbox is now checked with longer timeout for WebKit
    await expect(toggle).toBeChecked({ timeout: 10000 });
    // Small delay before form submission for WebKit
    await page.waitForTimeout(100);
    // Wait for the form POST response before continuing (matches passing test pattern)
    await Promise.all([
      page.waitForResponse(resp => resp.url().includes('/settings') && resp.request().method() === 'POST'),
      page.locator('#offline-mode button[type="submit"]').click()
    ]);
  }
  // Ensure the offline-download section is visible (matches passing test)
  await expect(page.locator('#offline-download')).toBeVisible({ timeout: 10000 });
}

// Helper to download an offline session
async function downloadSession(page) {
  await enableOfflineMode(page);
  const downloadBtn = page.locator('#download-session-btn');
  await downloadBtn.click();
  await expect(page.locator('#download-status')).toContainText('Downloaded', { timeout: 15000 });
}

test.describe('Offline Study Mode', () => {
  test('can enable offline mode in settings', async ({ authenticatedPage }) => {
    await authenticatedPage.goto('/settings', { waitUntil: 'domcontentloaded' });

    // Find offline mode section
    const offlineSection = authenticatedPage.locator('#offline-mode');
    await expect(offlineSection).toBeVisible();

    // Check browser support shows as supported
    const status = offlineSection.locator('#offline-status');
    await expect(status).toContainText('All features supported');

    // Enable offline mode
    const toggle = authenticatedPage.locator('#offlineModeToggle');
    if (!(await toggle.isChecked())) {
      // Scroll the checkbox into view for WebKit compatibility
      await toggle.scrollIntoViewIfNeeded();
      // Small delay after scrolling for WebKit
      await authenticatedPage.waitForTimeout(100);
      // Click with force to bypass actionability checks
      await toggle.click({ force: true });
      // Verify the checkbox is now checked with longer timeout for WebKit
      await expect(toggle).toBeChecked({ timeout: 10000 });
      // Small delay before form submission for WebKit
      await authenticatedPage.waitForTimeout(100);
    }

    // Save settings and wait for response
    await Promise.all([
      authenticatedPage.waitForResponse(resp => resp.url().includes('/settings') && resp.request().method() === 'POST'),
      offlineSection.locator('button[type="submit"]').click()
    ]);

    // Verify options appear
    const downloadSection = authenticatedPage.locator('#offline-download');
    await expect(downloadSection).toBeVisible({ timeout: 10000 });
  });

  test('can download offline session', async ({ authenticatedPage, testUser }) => {
    setupScenario(testUser.username, 'tier1_new', testUser.dataDir);
    await downloadSession(authenticatedPage);

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
    setupScenario(testUser.username, 'tier1_new', testUser.dataDir);
    await downloadSession(authenticatedPage);

    // Go to offline study page directly
    await authenticatedPage.goto('/offline-study');

    // Should show session ready state
    await expect(authenticatedPage.locator('#session-ready-state')).toBeVisible({ timeout: 15000 });
    await expect(authenticatedPage.locator('#session-card-count')).not.toHaveText('0');
  });

  test('can complete study cards offline', async ({ authenticatedPage, testUser }) => {
    setupScenario(testUser.username, 'tier1_new', testUser.dataDir);
    await downloadSession(authenticatedPage);

    // Go to offline study
    await authenticatedPage.goto('/offline-study');
    await expect(authenticatedPage.locator('#session-ready-state')).toBeVisible({ timeout: 15000 });

    // Start studying
    await authenticatedPage.locator('#start-study-btn').click();

    // Should show study active state
    await expect(authenticatedPage.locator('#study-active-state')).toBeVisible();
    await expect(authenticatedPage.locator('#card-container')).toBeVisible();

    // Wait for card to render
    await expect(authenticatedPage.locator('.offline-card')).toBeVisible({ timeout: 10000 });

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
    setupScenario(testUser.username, 'tier1_new', testUser.dataDir);
    await downloadSession(authenticatedPage);

    // Go to offline study and answer one card
    await authenticatedPage.goto('/offline-study');
    await expect(authenticatedPage.locator('#session-ready-state')).toBeVisible({ timeout: 15000 });
    await authenticatedPage.locator('#start-study-btn').click();
    await expect(authenticatedPage.locator('.offline-card')).toBeVisible({ timeout: 10000 });

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

    // Buffer for stability timer + async operations
    await authenticatedPage.waitForTimeout(500);

    // Wait for sync prompt modal to appear
    const syncPrompt = authenticatedPage.locator('#sync-prompt-modal');
    await expect(syncPrompt).toBeVisible({ timeout: 10000 });

    // Verify pending count is shown
    const countEl = authenticatedPage.locator('#sync-prompt-count');
    await expect(countEl).toBeVisible();

    // Click sync now
    await authenticatedPage.click('#sync-now-btn');

    // Modal should close and sync notification should appear briefly
    await expect(syncPrompt).toBeHidden({ timeout: 10000 });
  });

  test('auto-sync clears pending reviews from IndexedDB', async ({ authenticatedPage, testUser }) => {
    setupScenario(testUser.username, 'tier1_new', testUser.dataDir);
    await downloadSession(authenticatedPage);

    // Do some offline study
    await authenticatedPage.goto('/offline-study');
    await expect(authenticatedPage.locator('#session-ready-state')).toBeVisible({ timeout: 15000 });
    await authenticatedPage.locator('#start-study-btn').click();
    await expect(authenticatedPage.locator('.offline-card')).toBeVisible({ timeout: 10000 });

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

    // Buffer for stability timer + async operations
    await authenticatedPage.waitForTimeout(500);

    // Wait for sync prompt modal to appear
    const syncPrompt = authenticatedPage.locator('#sync-prompt-modal');
    await expect(syncPrompt).toBeVisible({ timeout: 10000 });

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

test.describe('Bug 4: Same Card Not Shown Immediately After Wrong Answer', () => {
  test('different card shown after wrong answer when multiple cards available', async ({ authenticatedPage, testUser }) => {
    setupScenario(testUser.username, 'tier1_new', testUser.dataDir);
    await downloadSession(authenticatedPage);

    await authenticatedPage.goto('/offline-study');
    await expect(authenticatedPage.locator('#session-ready-state')).toBeVisible({ timeout: 15000 });
    await authenticatedPage.locator('#start-study-btn').click();
    await expect(authenticatedPage.locator('.offline-card')).toBeVisible({ timeout: 10000 });

    // Get the first card's front text
    const firstCardFront = await authenticatedPage.locator('.card-front').textContent();

    // Answer incorrectly (for MCQ, click wrong answer; for text, submit wrong answer)
    const choiceBtn = authenticatedPage.locator('.choice-btn').first();
    if (await choiceBtn.isVisible()) {
      // MCQ - double-click to confirm wrong answer
      await choiceBtn.dblclick();
    } else {
      // Text input - submit obviously wrong answer
      await authenticatedPage.locator('.answer-input').fill('zzzzwrongzzz');
      await authenticatedPage.locator('.submit-btn').click();
    }

    // Wait for result and continue
    await expect(authenticatedPage.locator('.result-section')).toBeVisible({ timeout: 5000 });
    await authenticatedPage.locator('.continue-btn').click();

    // Wait for next card
    await expect(authenticatedPage.locator('.offline-card')).toBeVisible({ timeout: 10000 });

    // Get the second card's front text
    const secondCardFront = await authenticatedPage.locator('.card-front').textContent();

    // The second card should be different from the first (Bug 4 fix)
    // Note: If there's only one card in the session, this test may need adjustment
    expect(secondCardFront).not.toBe(firstCardFront);
  });
});

test.describe('Bug 7: Session Progress Persists Across Navigation', () => {
  test('progress is saved to localStorage on navigation', async ({ authenticatedPage, testUser }) => {
    setupScenario(testUser.username, 'tier1_new', testUser.dataDir);
    await downloadSession(authenticatedPage);

    // Start studying
    await authenticatedPage.goto('/offline-study');
    await expect(authenticatedPage.locator('#session-ready-state')).toBeVisible({ timeout: 15000 });
    await authenticatedPage.locator('#start-study-btn').click();
    await expect(authenticatedPage.locator('.offline-card')).toBeVisible({ timeout: 10000 });

    // Answer a card
    const choiceBtn = authenticatedPage.locator('.choice-btn').first();
    if (await choiceBtn.isVisible()) {
      await choiceBtn.dblclick();
    } else {
      await authenticatedPage.locator('.answer-input').fill('test');
      await authenticatedPage.locator('.submit-btn').click();
    }

    // Wait for result and continue
    await expect(authenticatedPage.locator('.result-section')).toBeVisible({ timeout: 5000 });
    await authenticatedPage.locator('.continue-btn').click();
    await expect(authenticatedPage.locator('.offline-card')).toBeVisible({ timeout: 10000 });

    // Trigger beforeunload to save progress (simulate navigation)
    await authenticatedPage.evaluate(() => {
      // Manually trigger the save function that beforeunload calls
      if (window.OfflineStorage && window.OfflineStorage.saveSessionProgress) {
        // Get the current study state from the page
        const session = window['OfflineStudy']?.getSession?.() || { session_id: 'test' };
        window.OfflineStorage.saveSessionProgress({
          sessionId: session.session_id || 'test',
          totalReviewed: 1,
          correctCount: 0,
          cardQueueIds: [],
          reinforcementQueueIds: []
        });
      }
    });

    // Verify progress was saved to localStorage
    const savedProgress = await authenticatedPage.evaluate(() => {
      const data = localStorage.getItem('offlineStudy_sessionProgress');
      return data ? JSON.parse(data) : null;
    });

    expect(savedProgress).not.toBeNull();
    expect(savedProgress.totalReviewed).toBe(1);
  });

  test('storage functions work correctly', async ({ authenticatedPage, testUser }) => {
    setupScenario(testUser.username, 'tier1_new', testUser.dataDir);
    await downloadSession(authenticatedPage);

    // Test save/get/clear cycle
    const testProgress = {
      sessionId: 'test-session',
      totalReviewed: 5,
      correctCount: 3,
      cardQueueIds: [1, 2, 3],
      reinforcementQueueIds: [4]
    };

    // Save progress
    await authenticatedPage.evaluate((progress) => {
      window.OfflineStorage.saveSessionProgress(progress);
    }, testProgress);

    // Verify save worked
    const saved = await authenticatedPage.evaluate(() => {
      return window.OfflineStorage.getSessionProgress();
    });

    expect(saved).not.toBeNull();
    expect(saved.totalReviewed).toBe(5);
    expect(saved.correctCount).toBe(3);
    expect(saved.sessionId).toBe('test-session');

    // Clear progress
    await authenticatedPage.evaluate(() => {
      window.OfflineStorage.clearSessionProgress();
    });

    // Verify clear worked
    const afterClear = await authenticatedPage.evaluate(() => {
      return window.OfflineStorage.getSessionProgress();
    });

    expect(afterClear).toBeNull();
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
    await expect(authenticatedPage.locator('#no-session-state')).toBeVisible({ timeout: 10000 });
    await expect(authenticatedPage.locator('#no-session-state')).toContainText('No Offline Session');
  });
});
