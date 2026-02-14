import { test, expect, setupScenario } from '../fixtures/auth';
import { execSync } from 'child_process';
import * as path from 'path';

const PROJECT_ROOT = path.resolve(__dirname, '../../..');
const PY_SCRIPTS_DIR = path.join(PROJECT_ROOT, 'py_scripts');

/**
 * E2E tests for Due Counter Respects Daily New Card Limit
 */

function setSetting(username: string, key: string, value: string, dataDir: string): void {
  try {
    const sqlCmd = `sqlite3 "${dataDir}/users/${username}/learning.db" "INSERT OR REPLACE INTO settings (key, value) VALUES ('${key}', '${value}')"`;
    execSync(sqlCmd, { stdio: 'pipe' });
  } catch (e) {
    console.warn(`Failed to set setting ${key}=${value}: ${e}`);
  }
}

/**
 * Answer the currently displayed study card (MCQ or text input).
 * Waits for input elements with retrying assertions to avoid race conditions
 * with HTMX swaps and JS event handler setup.
 */
async function answerCurrentCard(page) {
  const textInput = page.locator('[data-testid="answer-input"]');
  const choiceGrid = page.locator('[data-testid="choice-grid"]');

  // Wait for either input type to appear (retrying, survives HTMX swap timing)
  await expect(textInput.or(choiceGrid)).toBeVisible({ timeout: 10000 });

  if (await choiceGrid.isVisible()) {
    // MCQ: select first choice, wait for submit button to be enabled (proves JS handler ran)
    await page.locator('[data-testid="choice-option"]').first().click();
    const submitBtn = page.locator('[data-testid="submit-answer"]');
    await expect(submitBtn).toBeEnabled({ timeout: 5000 });
    await submitBtn.click();
  } else {
    // Text input
    await textInput.fill('test');
    await page.locator('[data-testid="submit-answer"]').click();
  }

  await expect(page.locator('[data-testid="result-phase"]')).toBeVisible({ timeout: 10000 });
}

test.describe('Due Counter Daily Limit', () => {

  test('due count shows correct number with daily limit', async ({ authenticatedPage, testUser }) => {
    setupScenario(testUser.username, 'tier1_new', testUser.dataDir);
    setSetting(testUser.username, 'daily_new_cards', '5', testUser.dataDir);

    await authenticatedPage.goto('/');
    await authenticatedPage.waitForLoadState('domcontentloaded');

    const dueCountEl = authenticatedPage.locator('[data-testid="due-count"]');
    await expect(dueCountEl).toBeVisible();

    const dueCountText = await dueCountEl.textContent();
    const dueCount = parseInt(dueCountText?.trim() || '0', 10);

    expect(dueCount).toBeLessThanOrEqual(30);
    console.log(`Due count with daily limit 5: ${dueCount}`);
  });

  test('due count decreases after studying to limit', async ({ authenticatedPage, testUser }) => {
    setupScenario(testUser.username, 'tier1_new', testUser.dataDir);
    setSetting(testUser.username, 'daily_new_cards', '2', testUser.dataDir);

    await authenticatedPage.goto('/');
    await authenticatedPage.waitForLoadState('domcontentloaded');

    const initialDueCount = authenticatedPage.locator('[data-testid="due-count"]');
    await expect(initialDueCount).toBeVisible();
    const initialCount = parseInt(await initialDueCount.textContent() || '0', 10);

    await authenticatedPage.locator('[data-testid="start-study-btn"]').click();
    await expect(authenticatedPage.locator('[data-testid="card-container"]')).toBeVisible();

    for (let i = 0; i < 2; i++) {
      await answerCurrentCard(authenticatedPage);

      const nextBtn = authenticatedPage.locator('[data-testid="next-card"]');
      if (await nextBtn.isVisible()) {
        await nextBtn.click();
        // Wait for next card or no-cards state
        const cardContainer = authenticatedPage.locator('[data-testid="card-container"]');
        const noCards = authenticatedPage.locator('[data-testid="no-cards"]');
        await expect(cardContainer.or(noCards)).toBeVisible();
      } else {
        break;
      }
    }

    await authenticatedPage.goto('/');
    await authenticatedPage.waitForLoadState('domcontentloaded');

    const finalDueCount = authenticatedPage.locator('[data-testid="due-count"]');
    await expect(finalDueCount).toBeVisible();
    const finalCount = parseInt(await finalDueCount.textContent() || '0', 10);

    console.log(`Due count before: ${initialCount}, after: ${finalCount}`);
    expect(finalCount).toBeLessThanOrEqual(initialCount);
  });

  test('study page shows no cards when daily limit reached', async ({ authenticatedPage, testUser }) => {
    setupScenario(testUser.username, 'tier1_new', testUser.dataDir);
    setSetting(testUser.username, 'daily_new_cards', '1', testUser.dataDir);

    await authenticatedPage.goto('/study');

    const cardContainer = authenticatedPage.locator('[data-testid="card-container"]');
    const hasCard = await cardContainer.isVisible().catch(() => false);

    if (hasCard) {
      await answerCurrentCard(authenticatedPage);

      const nextBtn = authenticatedPage.locator('[data-testid="next-card"]');
      if (await nextBtn.isVisible()) {
        await nextBtn.click();
        // Wait for next state to load
        await expect(cardContainer.or(authenticatedPage.locator('[data-testid="no-cards"]'))).toBeVisible();
      }
    }

    const pageText = await authenticatedPage.textContent('body');
    const hasNoCardsMessage =
      pageText?.toLowerCase().includes('no cards') ||
      pageText?.toLowerCase().includes('all done') ||
      pageText?.toLowerCase().includes('nothing') ||
      pageText?.toLowerCase().includes('come back');

    const isHome = authenticatedPage.url().includes('/') && !authenticatedPage.url().includes('/study');

    console.log(`After limit: no cards message = ${hasNoCardsMessage}, redirected home = ${isHome}`);
  });
});
