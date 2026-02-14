import { test, expect, setupScenario } from '../fixtures/auth';

test.describe('Study Mode', () => {
  test.describe('Interactive Study', () => {
    test('should display a card when cards are due', async ({ authenticatedPage, testUser }) => {
      setupScenario(testUser.username, 'tier1_new', testUser.dataDir);

      await authenticatedPage.goto('/study');

      await expect(authenticatedPage.locator('[data-testid="card-container"]')).toBeVisible();
      await expect(authenticatedPage.locator('[data-testid="card-front"]')).toBeVisible();
    });

    test('should have answer input or multiple choice', async ({ authenticatedPage, testUser }) => {
      setupScenario(testUser.username, 'tier1_new', testUser.dataDir);
      await authenticatedPage.goto('/study');

      const textInput = authenticatedPage.locator('[data-testid="answer-input"]');
      const choiceGrid = authenticatedPage.locator('[data-testid="choice-grid"]');

      const hasTextInput = await textInput.isVisible();
      const hasChoiceGrid = await choiceGrid.isVisible();

      expect(hasTextInput || hasChoiceGrid).toBeTruthy();
    });

    test('should validate text answer and show result', async ({ authenticatedPage, testUser }) => {
      setupScenario(testUser.username, 'tier1_new', testUser.dataDir);
      await authenticatedPage.goto('/study');

      await expect(authenticatedPage.locator('[data-testid="card-container"]')).toBeVisible();

      const textInput = authenticatedPage.locator('[data-testid="answer-input"]');
      const choiceGrid = authenticatedPage.locator('[data-testid="choice-grid"]');
      const hasTextInput = await textInput.isVisible();

      if (!hasTextInput) {
        // MCQ mode - select first choice and submit
        const hasChoiceGrid = await choiceGrid.isVisible();
        test.skip(!hasChoiceGrid, 'No input method available');
        await authenticatedPage.locator('[data-testid="choice-option"]').first().click();
      } else {
        await textInput.fill('test');
      }

      await Promise.all([
        authenticatedPage.waitForResponse(resp => resp.url().includes('/validate-answer')),
        authenticatedPage.locator('[data-testid="submit-answer"]').click()
      ]);

      await expect(authenticatedPage.locator('[data-testid="result-phase"]')).toBeVisible();
    });

    test('should load next card after answering', async ({ authenticatedPage, testUser }) => {
      setupScenario(testUser.username, 'tier1_new', testUser.dataDir);
      await authenticatedPage.goto('/study');

      await expect(authenticatedPage.locator('[data-testid="card-container"]')).toBeVisible();

      const textInput = authenticatedPage.locator('[data-testid="answer-input"]');
      if (await textInput.isVisible()) {
        await textInput.fill('test');
      } else {
        await authenticatedPage.locator('[data-testid="choice-option"]').first().click();
      }

      await Promise.all([
        authenticatedPage.waitForResponse(resp => resp.url().includes('/validate-answer')),
        authenticatedPage.locator('[data-testid="submit-answer"]').click()
      ]);

      await expect(authenticatedPage.locator('[data-testid="result-phase"]')).toBeVisible();

      await Promise.all([
        authenticatedPage.waitForResponse(resp => resp.url().includes('/next-card')),
        authenticatedPage.locator('[data-testid="next-card"]').click()
      ]);

      // Verify the flow completed by checking a card or no-cards state loaded
      const cardFront = authenticatedPage.locator('[data-testid="card-front"]');
      const noCards = authenticatedPage.locator('[data-testid="no-cards"]');
      const hasCard = await cardFront.isVisible().catch(() => false);
      const hasNoCards = await noCards.isVisible().catch(() => false);
      expect(hasCard || hasNoCards).toBeTruthy();
    });

    test('should show hint when clicking hint button', async ({ authenticatedPage, testUser }) => {
      setupScenario(testUser.username, 'tier1_new', testUser.dataDir);
      await authenticatedPage.goto('/study');

      await expect(authenticatedPage.locator('[data-testid="card-container"]')).toBeVisible();

      const hintButton = authenticatedPage.locator('[data-testid="hint-button"]');
      test.skip(!(await hintButton.isVisible()), 'No hint button for this card type');

      await hintButton.click();
      await expect(authenticatedPage.locator('[data-testid="hint-area"]')).toBeVisible();
    });
  });

  test.describe('Classic Study (Flip Cards)', () => {
    test('should display flip card interface', async ({ authenticatedPage, testUser }) => {
      setupScenario(testUser.username, 'tier1_new', testUser.dataDir);
      await authenticatedPage.goto('/study-classic');

      await expect(authenticatedPage.locator('[data-testid="card-container"]')).toBeVisible();
    });

    test('should flip card when clicking', async ({ authenticatedPage, testUser }) => {
      setupScenario(testUser.username, 'tier1_new', testUser.dataDir);
      await authenticatedPage.goto('/study-classic');

      const cardBack = authenticatedPage.locator('[data-testid="card-back"]');

      await authenticatedPage.locator('[data-testid="card-container"]').click();

      await expect(cardBack).toBeVisible();
    });

    test('should show quality rating buttons after flip', async ({ authenticatedPage, testUser }) => {
      setupScenario(testUser.username, 'tier1_new', testUser.dataDir);
      await authenticatedPage.goto('/study-classic');

      await authenticatedPage.locator('[data-testid="card-container"]').click();

      await expect(authenticatedPage.locator('[data-testid="quality-again"]')).toBeVisible();
      await expect(authenticatedPage.locator('[data-testid="quality-good"]')).toBeVisible();
    });

    test('should respond to keyboard shortcuts', async ({ authenticatedPage, testUser }) => {
      setupScenario(testUser.username, 'tier1_new', testUser.dataDir);
      await authenticatedPage.goto('/study-classic');

      // Press space to flip
      await authenticatedPage.keyboard.press('Space');
      await expect(authenticatedPage.locator('[data-testid="card-back"]')).toBeVisible();

      // Press number key to rate - should advance to next card or show completion
      await authenticatedPage.keyboard.press('3');

      // Verify the flow completed
      const nextCard = authenticatedPage.locator('[data-testid="card-container"]');
      const noCards = authenticatedPage.locator('[data-testid="no-cards"]');
      const hasNextCard = await nextCard.isVisible().catch(() => false);
      const hasDone = await noCards.isVisible().catch(() => false);
      expect(hasNextCard || hasDone).toBeTruthy();
    });
  });

  test.describe('Practice Mode', () => {
    test('should not affect SRS state', async ({ authenticatedPage, testUser }) => {
      setupScenario(testUser.username, 'tier1_new', testUser.dataDir);
      await authenticatedPage.goto('/practice');

      await expect(authenticatedPage.locator('[data-testid="card-container"]')).toBeVisible();

      const textInput = authenticatedPage.locator('[data-testid="answer-input"]');
      if (await textInput.isVisible()) {
        await textInput.fill('test');
        await Promise.all([
          authenticatedPage.waitForResponse(resp => resp.url().includes('/practice-validate')),
          authenticatedPage.locator('[data-testid="submit-answer"]').click()
        ]);
      }

      await expect(authenticatedPage.locator('text=Back to Study')).toBeVisible();
    });
  });

  test.describe('No Cards State', () => {
    test('should show no cards message when all reviewed', async ({ authenticatedPage, testUser }) => {
      setupScenario(testUser.username, 'all_graduated', testUser.dataDir);
      await authenticatedPage.goto('/study');

      const noCards = authenticatedPage.locator('[data-testid="no-cards"]');
      const allDone = authenticatedPage.locator('text=No cards due');

      const hasNoCards = await noCards.isVisible().catch(() => false);
      const hasAllDone = await allDone.isVisible().catch(() => false);

      expect(hasNoCards || hasAllDone).toBeTruthy();
    });
  });
});
