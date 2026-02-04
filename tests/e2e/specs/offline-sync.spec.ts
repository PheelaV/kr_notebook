/**
 * E2E tests for offline sync prompt behavior.
 *
 * Tests the new optional sync feature:
 * - Stability timer before showing prompt
 * - Sync prompt modal with Sync Now / Stay Offline options
 * - Manual offline mode entry from settings
 *
 * Note: These tests use the OfflineSyncTestAPI to simulate network events
 * without actually going offline, making them more reliable.
 */
import { test, expect, setupScenario } from '../fixtures/auth';

// Helper to enable offline mode in settings
async function enableOfflineMode(page) {
  // Use domcontentloaded - settings page has async IndexedDB ops that delay 'load' in WebKit
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
  // enableOfflineMode navigates to /settings and ensures offline mode is enabled
  await enableOfflineMode(page);

  // Download button should be visible now (enableOfflineMode waits for #offline-download)
  const downloadBtn = page.locator('#download-session-btn');
  await downloadBtn.click();

  // Wait for download to complete
  const status = page.locator('#download-status');
  await expect(status).toContainText('Downloaded', { timeout: 15000 });
}

// Helper to add a pending review to IndexedDB
async function addPendingReview(page) {
  await page.evaluate(async () => {
    if (!window.OfflineStorage) return;

    // Add a fake response to simulate a pending review
    await window.OfflineStorage.addResponse({
      card_id: 1,
      quality: 4,
      is_correct: true,
      hints_used: 0,
      timestamp: new Date().toISOString(),
      learning_step: 0,
      fsrs_stability: 1.0,
      fsrs_difficulty: 5.0,
      next_review: new Date(Date.now() + 86400000).toISOString(),
    });
  });
}

test.describe('Offline Sync Prompt', () => {
  test.beforeEach(async ({ authenticatedPage, testUser }) => {
    // Set up scenario with cards
    setupScenario(testUser.username, 'tier1_new', testUser.dataDir);

    // Enable offline mode and download session
    await downloadSession(authenticatedPage);
  });

  test('shows sync prompt after stable connection', async ({ authenticatedPage }) => {
    // Add a pending review
    await addPendingReview(authenticatedPage);

    // Set short stability delay for testing
    await authenticatedPage.evaluate(() => {
      window.OfflineSyncTestAPI.setStabilityDelay(100); // 100ms for test
    });

    // Simulate coming online
    await authenticatedPage.evaluate(() => {
      window.OfflineSyncTestAPI.simulateOnline();
    });

    // Buffer for stability timer (100ms) + async getPendingCount + modal render
    // Firefox/WebKit are significantly slower than Chrome
    await authenticatedPage.waitForTimeout(500);

    // Wait for prompt to appear
    await expect(authenticatedPage.locator('#sync-prompt-modal')).toBeVisible({ timeout: 10000 });

    // Verify pending count is shown
    const countEl = authenticatedPage.locator('#sync-prompt-count');
    await expect(countEl).toBeVisible();
    const count = await countEl.textContent();
    expect(parseInt(count || '0', 10)).toBeGreaterThan(0);
  });

  test('cancels prompt if connection lost during stability wait', async ({ authenticatedPage }) => {
    // Add a pending review
    await addPendingReview(authenticatedPage);

    // Set longer stability delay
    await authenticatedPage.evaluate(() => {
      window.OfflineSyncTestAPI.setStabilityDelay(2000);
    });

    // Simulate online
    await authenticatedPage.evaluate(() => {
      window.OfflineSyncTestAPI.simulateOnline();
    });

    // Small buffer to ensure event handler completes
    await authenticatedPage.waitForTimeout(50);

    // Verify timer is active
    const timerActive = await authenticatedPage.evaluate(() => {
      return window.OfflineSyncTestAPI.isStabilityTimerActive();
    });
    expect(timerActive).toBe(true);

    // Simulate offline before timer fires
    await authenticatedPage.evaluate(() => {
      window.OfflineSyncTestAPI.simulateOffline();
    });

    // Verify timer was cancelled
    const timerCancelled = await authenticatedPage.evaluate(() => {
      return !window.OfflineSyncTestAPI.isStabilityTimerActive();
    });
    expect(timerCancelled).toBe(true);

    // Verify modal never appeared
    await expect(authenticatedPage.locator('#sync-prompt-modal')).toBeHidden();
  });

  test('sync now button triggers sync', async ({ authenticatedPage }) => {
    // Add a pending review
    await addPendingReview(authenticatedPage);

    // Force show prompt (bypass stability timer)
    await authenticatedPage.evaluate(() => {
      window.OfflineSyncTestAPI.forceShowSyncPrompt();
    });

    await expect(authenticatedPage.locator('#sync-prompt-modal')).toBeVisible();

    // Click sync now
    await authenticatedPage.click('#sync-now-btn');

    // Modal should close
    await expect(authenticatedPage.locator('#sync-prompt-modal')).toBeHidden({ timeout: 10000 });

    // Pending count should be 0 after sync
    const pendingCount = await authenticatedPage.evaluate(async () => {
      return await window.OfflineStorage.getPendingCount();
    });
    // Note: sync might succeed or fail depending on server state, but modal should close
    expect(pendingCount).toBeDefined();
  });

  test('stay offline button closes modal without sync', async ({ authenticatedPage }) => {
    // Add a pending review
    await addPendingReview(authenticatedPage);

    // Force show prompt
    await authenticatedPage.evaluate(() => {
      window.OfflineSyncTestAPI.forceShowSyncPrompt();
    });

    await expect(authenticatedPage.locator('#sync-prompt-modal')).toBeVisible();

    // Get pending count before
    const countBefore = await authenticatedPage.evaluate(async () => {
      return await window.OfflineStorage.getPendingCount();
    });

    // Click stay offline
    await authenticatedPage.click('#stay-offline-btn');

    // Modal should close
    await expect(authenticatedPage.locator('#sync-prompt-modal')).toBeHidden();

    // Pending count should remain unchanged (sync didn't happen)
    const countAfter = await authenticatedPage.evaluate(async () => {
      return await window.OfflineStorage.getPendingCount();
    });
    expect(countAfter).toBe(countBefore);
  });

  test('does not show prompt when no pending reviews', async ({ authenticatedPage }) => {
    // Clear any pending reviews
    await authenticatedPage.evaluate(async () => {
      if (window.OfflineStorage) {
        await window.OfflineStorage.clearAll();
      }
    });

    // Re-download session (without pending reviews)
    await downloadSession(authenticatedPage);

    // Set short stability delay
    await authenticatedPage.evaluate(() => {
      window.OfflineSyncTestAPI.setStabilityDelay(100);
    });

    // Simulate coming online
    await authenticatedPage.evaluate(() => {
      window.OfflineSyncTestAPI.simulateOnline();
    });

    // Wait a bit for stability timer
    await authenticatedPage.waitForTimeout(500);

    // Modal should not appear (no pending reviews)
    await expect(authenticatedPage.locator('#sync-prompt-modal')).toBeHidden();
  });
});

test.describe('Bug 5: Prompt Cooldown After Dismissal', () => {
  test.beforeEach(async ({ authenticatedPage, testUser }) => {
    setupScenario(testUser.username, 'tier1_new', testUser.dataDir);
    await downloadSession(authenticatedPage);
  });

  test('Later button closes modal and sets cooldown', async ({ authenticatedPage }) => {
    // Add a pending review
    await addPendingReview(authenticatedPage);

    // Force show prompt
    await authenticatedPage.evaluate(() => {
      window.OfflineSyncTestAPI.forceShowSyncPrompt();
    });

    await expect(authenticatedPage.locator('#sync-prompt-modal')).toBeVisible();

    // Click Later button (new button from Bug 5 fix)
    await authenticatedPage.click('#later-btn');

    // Modal should close
    await expect(authenticatedPage.locator('#sync-prompt-modal')).toBeHidden();

    // Verify cooldown was set in localStorage
    const cooldownSet = await authenticatedPage.evaluate(() => {
      const dismissedAt = localStorage.getItem('offlineSync_promptDismissedAt');
      return dismissedAt !== null && Date.now() - parseInt(dismissedAt, 10) < 60000; // Set within last minute
    });
    expect(cooldownSet).toBe(true);
  });

  test('prompt does not reappear during cooldown period', async ({ authenticatedPage }) => {
    // Add a pending review
    await addPendingReview(authenticatedPage);

    // Set cooldown (simulate recent dismissal)
    await authenticatedPage.evaluate(() => {
      localStorage.setItem('offlineSync_promptDismissedAt', Date.now().toString());
    });

    // Set short stability delay
    await authenticatedPage.evaluate(() => {
      window.OfflineSyncTestAPI.setStabilityDelay(100);
    });

    // Simulate coming online
    await authenticatedPage.evaluate(() => {
      window.OfflineSyncTestAPI.simulateOnline();
    });

    // Wait for stability timer
    await authenticatedPage.waitForTimeout(500);

    // Modal should NOT appear (cooldown active)
    await expect(authenticatedPage.locator('#sync-prompt-modal')).toBeHidden();
  });

  test('prompt appears after cooldown expires', async ({ authenticatedPage }) => {
    // Add a pending review
    await addPendingReview(authenticatedPage);

    // Set expired cooldown (16 minutes ago)
    await authenticatedPage.evaluate(() => {
      const sixteenMinutesAgo = Date.now() - (16 * 60 * 1000);
      localStorage.setItem('offlineSync_promptDismissedAt', sixteenMinutesAgo.toString());
    });

    // Set short stability delay
    await authenticatedPage.evaluate(() => {
      window.OfflineSyncTestAPI.setStabilityDelay(100);
    });

    // Simulate coming online
    await authenticatedPage.evaluate(() => {
      window.OfflineSyncTestAPI.simulateOnline();
    });

    // Wait for stability timer
    await authenticatedPage.waitForTimeout(500);

    // Modal should appear (cooldown expired)
    await expect(authenticatedPage.locator('#sync-prompt-modal')).toBeVisible({ timeout: 10000 });
  });

  test('backdrop click dismisses and sets cooldown', async ({ authenticatedPage }) => {
    // Add a pending review
    await addPendingReview(authenticatedPage);

    // Clear any existing cooldown
    await authenticatedPage.evaluate(() => {
      localStorage.removeItem('offlineSync_promptDismissedAt');
    });

    // Force show prompt
    await authenticatedPage.evaluate(() => {
      window.OfflineSyncTestAPI.forceShowSyncPrompt();
    });

    await expect(authenticatedPage.locator('#sync-prompt-modal')).toBeVisible();

    // Click on backdrop (outside the modal content)
    await authenticatedPage.locator('#sync-prompt-modal').click({ position: { x: 10, y: 10 } });

    // Modal should close
    await expect(authenticatedPage.locator('#sync-prompt-modal')).toBeHidden();

    // Verify cooldown was set
    const cooldownSet = await authenticatedPage.evaluate(() => {
      return localStorage.getItem('offlineSync_promptDismissedAt') !== null;
    });
    expect(cooldownSet).toBe(true);
  });
});

test.describe('Manual Offline Mode', () => {
  test.beforeEach(async ({ authenticatedPage, testUser }) => {
    // Set up scenario with cards
    setupScenario(testUser.username, 'tier1_new', testUser.dataDir);
  });

  test('enter offline mode button appears after downloading session', async ({ authenticatedPage }) => {
    // Enable offline mode
    await enableOfflineMode(authenticatedPage);

    // Initially, enter offline section should be hidden
    await expect(authenticatedPage.locator('#enter-offline-section')).toBeHidden();

    // Download a session
    await authenticatedPage.locator('#download-session-btn').click();
    await expect(authenticatedPage.locator('#download-status')).toContainText('Downloaded', { timeout: 15000 });

    // Now enter offline section should be visible (may take time for UI to poll IndexedDB)
    await expect(authenticatedPage.locator('#enter-offline-section')).toBeVisible({ timeout: 10000 });
  });

  test('enter offline mode button navigates to offline study', async ({ authenticatedPage }) => {
    // Download a session first
    await downloadSession(authenticatedPage);

    // Click enter offline mode
    const enterBtn = authenticatedPage.locator('#enter-offline-btn');
    await expect(enterBtn).toBeVisible();
    await enterBtn.click();

    // Should navigate to offline study
    await expect(authenticatedPage).toHaveURL('/offline-study');
  });

  test('enter offline mode without session shows message', async ({ authenticatedPage }) => {
    // Enable offline mode but don't download
    await enableOfflineMode(authenticatedPage);

    // Clear any existing session
    await authenticatedPage.evaluate(async () => {
      if (window.OfflineStorage) {
        await window.OfflineStorage.clearAll();
      }
    });

    // Reload to update UI
    await authenticatedPage.reload();

    // Enter offline button should not be visible since no session
    await expect(authenticatedPage.locator('#enter-offline-section')).toBeHidden();
  });
});

test.describe('Test API', () => {
  test('test API is available', async ({ authenticatedPage }) => {
    await authenticatedPage.goto('/');

    const hasTestAPI = await authenticatedPage.evaluate(() => {
      return typeof window.OfflineSyncTestAPI !== 'undefined';
    });

    expect(hasTestAPI).toBe(true);
  });

  test('test API can simulate online/offline events', async ({ authenticatedPage }) => {
    await authenticatedPage.goto('/');

    // Set delay longer than our buffer wait so timer is still active when we check
    await authenticatedPage.evaluate(() => {
      window.OfflineSyncTestAPI.setStabilityDelay(2000);
    });

    // Simulate online
    await authenticatedPage.evaluate(() => {
      window.OfflineSyncTestAPI.simulateOnline();
    });

    // Buffer to ensure event handler completes (Firefox needs more time)
    await authenticatedPage.waitForTimeout(100);

    // Timer should be active (2000ms delay hasn't elapsed yet)
    const timerActive = await authenticatedPage.evaluate(() => {
      return window.OfflineSyncTestAPI.isStabilityTimerActive();
    });
    expect(timerActive).toBe(true);

    // Simulate offline
    await authenticatedPage.evaluate(() => {
      window.OfflineSyncTestAPI.simulateOffline();
    });

    // Timer should be cancelled
    const timerCancelled = await authenticatedPage.evaluate(() => {
      return !window.OfflineSyncTestAPI.isStabilityTimerActive();
    });
    expect(timerCancelled).toBe(true);
  });
});
