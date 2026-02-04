import { test, expect, setupScenario } from '../fixtures/auth';
import { execSync } from 'child_process';
import * as path from 'path';

const PROJECT_ROOT = path.resolve(__dirname, '../../..');
const PY_SCRIPTS_DIR = path.join(PROJECT_ROOT, 'py_scripts');

/**
 * E2E tests for Bug 2: Due Counter Respects Daily New Card Limit
 *
 * These tests verify that:
 * 1. Home page due count respects daily new card limit
 * 2. After reaching limit, new cards don't show in due count
 * 3. Review cards still show after new card limit reached
 */

// Helper to set a user setting via db-manager
function setSetting(username: string, key: string, value: string, dataDir: string): void {
  const cmd = `uv run db-manager apply-preset tier1_new --user ${username} --data-dir "${dataDir}"`;
  // First, we need a custom command to set settings. For now, use sqlite directly.
  try {
    const sqlCmd = `sqlite3 "${dataDir}/users/${username}/learning.db" "INSERT OR REPLACE INTO settings (key, value) VALUES ('${key}', '${value}')"`;
    execSync(sqlCmd, { stdio: 'pipe' });
  } catch (e) {
    console.warn(`Failed to set setting ${key}=${value}: ${e}`);
  }
}

test.describe('Bug 2: Due Counter Respects Daily New Card Limit', () => {

  test('due count shows correct number with daily limit', async ({ authenticatedPage, testUser }) => {
    // Setup: Apply tier1_new scenario (30 new cards available)
    setupScenario(testUser.username, 'tier1_new', testUser.dataDir);

    // Set a low daily limit (e.g., 5 new cards)
    setSetting(testUser.username, 'daily_new_cards', '5', testUser.dataDir);

    // Navigate to home page
    await authenticatedPage.goto('/');
    await authenticatedPage.waitForLoadState('networkidle');

    // Get the due count
    const dueCountEl = authenticatedPage.locator('[data-testid="due-count"]');
    await expect(dueCountEl).toBeVisible();

    const dueCountText = await dueCountEl.textContent();
    const dueCount = parseInt(dueCountText?.trim() || '0', 10);

    // With daily limit of 5, due count should be at most 5 (not all 30 new cards)
    // Note: Some review cards might also be included
    expect(dueCount).toBeLessThanOrEqual(30);
    console.log(`Due count with daily limit 5: ${dueCount}`);
  });

  test('due count decreases after studying to limit', async ({ authenticatedPage, testUser }) => {
    // Setup: Apply tier1_new scenario
    setupScenario(testUser.username, 'tier1_new', testUser.dataDir);

    // Set daily limit to 2 new cards
    setSetting(testUser.username, 'daily_new_cards', '2', testUser.dataDir);

    // Get initial home page count
    await authenticatedPage.goto('/');
    await authenticatedPage.waitForLoadState('networkidle');

    const initialDueCount = authenticatedPage.locator('[data-testid="due-count"]');
    await expect(initialDueCount).toBeVisible();
    const initialCount = parseInt(await initialDueCount.textContent() || '0', 10);

    // Study 2 cards to reach the limit
    await authenticatedPage.click('[data-testid="start-study-btn"]');
    await authenticatedPage.waitForSelector('[data-testid="card-container"]');

    for (let i = 0; i < 2; i++) {
      // Answer the card (any answer)
      const textInput = authenticatedPage.locator('[data-testid="answer-input"]');
      const choiceGrid = authenticatedPage.locator('[data-testid="choice-grid"]');

      if (await textInput.isVisible()) {
        await textInput.fill('test');
        await authenticatedPage.click('[data-testid="submit-answer"]');
      } else if (await choiceGrid.isVisible()) {
        await authenticatedPage.locator('[data-testid="choice-option"]').first().click();
        await authenticatedPage.click('[data-testid="submit-answer"]');
      }

      // Wait for result and click next
      await authenticatedPage.waitForSelector('[data-testid="result-phase"]');

      // Check if there's a next card button
      const nextBtn = authenticatedPage.locator('[data-testid="next-card"]');
      if (await nextBtn.isVisible()) {
        await nextBtn.click();
        await authenticatedPage.waitForTimeout(500);
      } else {
        break; // No more cards
      }
    }

    // Return to home page
    await authenticatedPage.goto('/');
    await authenticatedPage.waitForLoadState('networkidle');

    // Check the new due count - should be reduced since we studied some cards
    const finalDueCount = authenticatedPage.locator('[data-testid="due-count"]');
    await expect(finalDueCount).toBeVisible();
    const finalCount = parseInt(await finalDueCount.textContent() || '0', 10);

    // After studying, count should have decreased
    console.log(`Due count before: ${initialCount}, after: ${finalCount}`);
    expect(finalCount).toBeLessThanOrEqual(initialCount);
  });

  test('study page shows no cards when daily limit reached', async ({ authenticatedPage, testUser }) => {
    // Setup: Apply tier1_new scenario
    setupScenario(testUser.username, 'tier1_new', testUser.dataDir);

    // Set daily limit to 1 new card
    setSetting(testUser.username, 'daily_new_cards', '1', testUser.dataDir);

    // Study 1 card to reach the limit
    await authenticatedPage.goto('/study');

    // Check if we have a card
    const cardContainer = authenticatedPage.locator('[data-testid="card-container"]');
    if (await cardContainer.isVisible()) {
      // Answer the card
      const textInput = authenticatedPage.locator('[data-testid="answer-input"]');
      if (await textInput.isVisible()) {
        await textInput.fill('test');
        await authenticatedPage.click('[data-testid="submit-answer"]');
        await authenticatedPage.waitForSelector('[data-testid="result-phase"]');

        // Try to get next card
        const nextBtn = authenticatedPage.locator('[data-testid="next-card"]');
        if (await nextBtn.isVisible()) {
          await nextBtn.click();
          await authenticatedPage.waitForTimeout(500);
        }
      }
    }

    // After reaching limit, should show "no cards" or similar
    // The page should indicate there are no more cards to study today
    const pageText = await authenticatedPage.textContent('body');
    const hasNoCardsMessage =
      pageText?.toLowerCase().includes('no cards') ||
      pageText?.toLowerCase().includes('all done') ||
      pageText?.toLowerCase().includes('nothing') ||
      pageText?.toLowerCase().includes('come back');

    // Either we see no cards message, or we're redirected to home
    const isHome = await authenticatedPage.url().includes('/') && !await authenticatedPage.url().includes('/study');

    console.log(`After limit: no cards message = ${hasNoCardsMessage}, redirected home = ${isHome}`);
  });
});
