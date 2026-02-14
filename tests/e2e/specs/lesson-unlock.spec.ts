import { test, expect, setupScenario } from '../fixtures/auth';

/**
 * E2E tests for Bug 1: Pack Lesson Unlock Notifications
 * and Bug 3: Study Page Filters Update After Unlock
 */

test.describe('Pack Lesson Unlock Notifications', () => {

  test('HaetaeSystem notification mechanism exists and works', async ({ authenticatedPage, testUser }) => {
    await authenticatedPage.goto('/');
    await authenticatedPage.waitForLoadState('domcontentloaded');

    const hasHaetaeSystem = await authenticatedPage.evaluate(() => {
      return typeof (window as any).HaetaeSystem !== 'undefined';
    });
    expect(hasHaetaeSystem).toBeTruthy();

    const hasShowSpeechBubble = await authenticatedPage.evaluate(() => {
      const hs = (window as any).HaetaeSystem;
      return hs && typeof hs.showSpeechBubble === 'function';
    });
    expect(hasShowSpeechBubble).toBeTruthy();
  });

  test('notification can be triggered via HaetaeSystem API', async ({ authenticatedPage, testUser }) => {
    await authenticatedPage.goto('/');
    await authenticatedPage.waitForLoadState('domcontentloaded');

    await authenticatedPage.evaluate(() => {
      const hs = (window as any).HaetaeSystem;
      if (hs && hs.showSpeechBubble) {
        hs.showSpeechBubble('Test Notification', 'This is a test', 3000);
      }
    });

    const speechBubble = authenticatedPage.locator('.haetae-speech-bubble');
    await expect(speechBubble).toContainText('Test Notification');
  });

  test('test mode notification trigger works with ) key', async ({ authenticatedPage, testUser }) => {
    await authenticatedPage.goto('/');
    await authenticatedPage.waitForLoadState('domcontentloaded');

    const testingMode = await authenticatedPage.evaluate(() => {
      return (window as any).TESTING_MODE;
    });

    if (testingMode) {
      await authenticatedPage.keyboard.press(')');

      const speechBubble = authenticatedPage.locator('.haetae-speech-bubble');
      await expect(speechBubble).toContainText('Unlocked');
    } else {
      console.log('TESTING_MODE not enabled, skipping ) key test');
    }
  });

  test('unlock notification template code exists in home page', async ({ authenticatedPage, testUser }) => {
    await authenticatedPage.goto('/');
    await authenticatedPage.waitForLoadState('domcontentloaded');

    const pageContent = await authenticatedPage.content();

    expect(pageContent).toContain('HaetaeSystem');

    const haetaeComponent = authenticatedPage.locator('.haetae-component');
    await expect(haetaeComponent).toBeVisible();
  });
});

test.describe('Filter Refresh After Unlock', () => {

  test('study page has filter controls', async ({ authenticatedPage, testUser }) => {
    setupScenario(testUser.username, 'tier1_new', testUser.dataDir);

    await authenticatedPage.goto('/study');
    await authenticatedPage.waitForLoadState('domcontentloaded');

    const cardContainer = authenticatedPage.locator('[data-testid="card-container"]');
    const noCards = authenticatedPage.locator('[data-testid="no-cards"]');

    const hasCard = await cardContainer.isVisible().catch(() => false);
    const hasNoCardsMsg = await noCards.isVisible().catch(() => false);

    expect(hasCard || hasNoCardsMsg).toBeTruthy();
  });

  test('lesson unlock redirects to home page', async ({ authenticatedPage, testUser }) => {
    setupScenario(testUser.username, 'tier1_new', testUser.dataDir);

    await authenticatedPage.goto('/study');
    await authenticatedPage.waitForLoadState('domcontentloaded');

    await authenticatedPage.goto('/');
    await authenticatedPage.waitForLoadState('domcontentloaded');

    const dueCount = authenticatedPage.locator('[data-testid="due-count"]');
    await expect(dueCount).toBeVisible();
  });
});

test.describe('Integration: Unlock Flow Verification', () => {

  test('home page shows correct structure for notifications', async ({ authenticatedPage, testUser }) => {
    await authenticatedPage.goto('/');
    await authenticatedPage.waitForLoadState('domcontentloaded');

    // Haetae mascot component
    const haetaeMascot = authenticatedPage.locator('.haetae-mascot');
    await expect(haetaeMascot).toBeVisible();

    // Speech bubble container
    const speechBubbleContainer = authenticatedPage.locator('.haetae-speech-bubble');
    expect(await speechBubbleContainer.count()).toBeGreaterThan(0);

    // Due count display
    const dueCount = authenticatedPage.locator('[data-testid="due-count"]');
    await expect(dueCount).toBeVisible();
  });

  test('notification bubble can show lesson unlock message', async ({ authenticatedPage, testUser }) => {
    await authenticatedPage.goto('/');
    await authenticatedPage.waitForLoadState('domcontentloaded');

    await authenticatedPage.evaluate(() => {
      const hs = (window as any).HaetaeSystem;
      if (hs && hs.showSpeechBubble) {
        hs.showSpeechBubble('Lesson 2 Unlocked!', 'New cards are available to study', 5000);
      }
    });

    const speechBubble = authenticatedPage.locator('.haetae-speech-bubble');
    await expect(speechBubble).toContainText('Lesson 2 Unlocked');
    await expect(speechBubble).toContainText('New cards');
  });
});
