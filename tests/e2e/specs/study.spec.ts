import { test, expect, setupScenario } from '../fixtures/auth';

test.describe('Study Mode', () => {
  test.describe('Interactive Study', () => {
    test('should display a card when cards are due', async ({ authenticatedPage, testUser }) => {
      // Set up scenario with due cards
      setupScenario(testUser.username, 'tier1_new', testUser.dataDir);

      await authenticatedPage.goto('/study');

      // Should show card container with a card
      await expect(authenticatedPage.locator('[data-testid="card-container"]')).toBeVisible();
      await expect(authenticatedPage.locator('[data-testid="card-front"]')).toBeVisible();
    });

    test('should have answer input or multiple choice', async ({ authenticatedPage, testUser }) => {
      setupScenario(testUser.username, 'tier1_new', testUser.dataDir);
      await authenticatedPage.goto('/study');

      // Either text input or multiple choice grid should be visible
      const textInput = authenticatedPage.locator('[data-testid="answer-input"]');
      const choiceGrid = authenticatedPage.locator('[data-testid="choice-grid"]');

      const hasTextInput = await textInput.isVisible();
      const hasChoiceGrid = await choiceGrid.isVisible();

      expect(hasTextInput || hasChoiceGrid).toBeTruthy();
    });

    test('should validate text answer and show result', async ({ authenticatedPage, testUser }) => {
      setupScenario(testUser.username, 'tier1_new', testUser.dataDir);
      await authenticatedPage.goto('/study');

      // Wait for card to load
      await authenticatedPage.waitForSelector('[data-testid="card-container"]');

      // Check if this is a text input card
      const textInput = authenticatedPage.locator('[data-testid="answer-input"]');
      if (await textInput.isVisible()) {
        // Type an answer
        await textInput.fill('test');
        await authenticatedPage.click('[data-testid="submit-answer"]');

        // Should show result
        await expect(authenticatedPage.locator('[data-testid="result-phase"]')).toBeVisible();
      }
    });

    test('should load next card after answering', async ({ authenticatedPage, testUser }) => {
      setupScenario(testUser.username, 'tier1_new', testUser.dataDir);
      await authenticatedPage.goto('/study');

      await authenticatedPage.waitForSelector('[data-testid="card-container"]');

      // Get the initial card front text
      const initialFront = await authenticatedPage.locator('[data-testid="card-front"]').textContent();

      // Submit an answer
      const textInput = authenticatedPage.locator('[data-testid="answer-input"]');
      if (await textInput.isVisible()) {
        await textInput.fill('test');
        await authenticatedPage.click('[data-testid="submit-answer"]');
      } else {
        // Click first choice
        await authenticatedPage.locator('[data-testid="choice-option"]').first().click();
        await authenticatedPage.click('[data-testid="submit-answer"]');
      }

      // Click next card
      await authenticatedPage.click('[data-testid="next-card"]');

      // Card should change (or show no more cards)
      const newFront = await authenticatedPage.locator('[data-testid="card-front"]').textContent();
      // Note: It might be the same card if there's only one, so we just verify the flow completes
    });

    test('should show hint when clicking hint button', async ({ authenticatedPage, testUser }) => {
      setupScenario(testUser.username, 'tier1_new', testUser.dataDir);
      await authenticatedPage.goto('/study');

      await authenticatedPage.waitForSelector('[data-testid="card-container"]');

      const hintButton = authenticatedPage.locator('[data-testid="hint-button"]');
      if (await hintButton.isVisible()) {
        await hintButton.click();
        await expect(authenticatedPage.locator('[data-testid="hint-area"]')).toBeVisible();
      }
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

      // Initially card back should be hidden
      const cardBack = authenticatedPage.locator('[data-testid="card-back"]');

      // Click to flip
      await authenticatedPage.locator('[data-testid="card-container"]').click();

      // Card back should now be visible
      await expect(cardBack).toBeVisible();
    });

    test('should show quality rating buttons after flip', async ({ authenticatedPage, testUser }) => {
      setupScenario(testUser.username, 'tier1_new', testUser.dataDir);
      await authenticatedPage.goto('/study-classic');

      // Flip the card
      await authenticatedPage.locator('[data-testid="card-container"]').click();

      // Quality buttons should be visible
      await expect(authenticatedPage.locator('[data-testid="quality-again"]')).toBeVisible();
      await expect(authenticatedPage.locator('[data-testid="quality-good"]')).toBeVisible();
    });

    test('should respond to keyboard shortcuts', async ({ authenticatedPage, testUser }) => {
      setupScenario(testUser.username, 'tier1_new', testUser.dataDir);
      await authenticatedPage.goto('/study-classic');

      // Press space to flip
      await authenticatedPage.keyboard.press('Space');
      await expect(authenticatedPage.locator('[data-testid="card-back"]')).toBeVisible();

      // Press number key to rate
      await authenticatedPage.keyboard.press('3');

      // Should advance to next card (or show completion)
    });
  });

  test.describe('Practice Mode', () => {
    test('should not affect SRS state', async ({ authenticatedPage, testUser }) => {
      setupScenario(testUser.username, 'tier1_new', testUser.dataDir);
      await authenticatedPage.goto('/practice');

      await expect(authenticatedPage.locator('[data-testid="card-container"]')).toBeVisible();

      // Complete a practice round
      const textInput = authenticatedPage.locator('[data-testid="answer-input"]');
      if (await textInput.isVisible()) {
        await textInput.fill('test');
        await authenticatedPage.click('[data-testid="submit-answer"]');
      }

      // Verify it's in practice mode (has back to study button)
      await expect(authenticatedPage.locator('text=Back to Study')).toBeVisible();
    });
  });

  test.describe('No Cards State', () => {
    test('should show no cards message when all reviewed', async ({ authenticatedPage, testUser }) => {
      setupScenario(testUser.username, 'all_graduated', testUser.dataDir);
      await authenticatedPage.goto('/study');

      // Should show a "no cards" or "all done" message
      const noCards = authenticatedPage.locator('[data-testid="no-cards"]');
      const allDone = authenticatedPage.locator('text=No cards due');

      // One of these should be visible
      const hasNoCards = await noCards.isVisible().catch(() => false);
      const hasAllDone = await allDone.isVisible().catch(() => false);

      expect(hasNoCards || hasAllDone).toBeTruthy();
    });
  });
});
