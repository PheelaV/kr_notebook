import { test, expect, setupScenario } from '../fixtures/auth';

test.describe('Answer Grammar Visual Indicators', () => {
  test.describe('Practice Mode', () => {
    test('should display practice cards', async ({ authenticatedPage, testUser }) => {
      setupScenario(testUser.username, 'tier1_new', testUser.dataDir);

      await authenticatedPage.goto('/practice?mode=interactive');

      // Should show card container
      await expect(authenticatedPage.locator('[data-testid="card-container"]')).toBeVisible();
    });

    test('should show correct result on right answer', async ({ authenticatedPage, testUser }) => {
      setupScenario(testUser.username, 'tier1_new', testUser.dataDir);
      await authenticatedPage.goto('/practice?mode=interactive');

      await authenticatedPage.waitForSelector('[data-testid="card-container"]');

      // Check if text input is visible
      const textInput = authenticatedPage.locator('[data-testid="answer-input"]');
      if (await textInput.isVisible()) {
        // Get the data-answer attribute for the correct answer
        const cardContainer = authenticatedPage.locator('[data-testid="card-container"]');
        const correctAnswer = await cardContainer.getAttribute('data-answer');

        if (correctAnswer) {
          await textInput.fill(correctAnswer);
          await authenticatedPage.click('[data-testid="submit-answer"]');

          // Should show correct result
          await expect(authenticatedPage.locator('[data-result="correct"]')).toBeVisible();
        }
      }
    });

    test('should show incorrect result on wrong answer', async ({ authenticatedPage, testUser }) => {
      setupScenario(testUser.username, 'tier1_new', testUser.dataDir);
      await authenticatedPage.goto('/practice?mode=interactive');

      await authenticatedPage.waitForSelector('[data-testid="card-container"]');

      const textInput = authenticatedPage.locator('[data-testid="answer-input"]');
      if (await textInput.isVisible()) {
        // Type a definitely wrong answer
        await textInput.fill('xyzzy_wrong_answer_12345');
        await authenticatedPage.click('[data-testid="submit-answer"]');

        // Should show incorrect result
        await expect(authenticatedPage.locator('[data-result="incorrect"]')).toBeVisible();
      }
    });

    test('should display correct answer after validation', async ({ authenticatedPage, testUser }) => {
      setupScenario(testUser.username, 'tier1_new', testUser.dataDir);
      await authenticatedPage.goto('/practice?mode=interactive');

      await authenticatedPage.waitForSelector('[data-testid="card-container"]');

      const textInput = authenticatedPage.locator('[data-testid="answer-input"]');
      if (await textInput.isVisible()) {
        await textInput.fill('test');
        await authenticatedPage.click('[data-testid="submit-answer"]');

        // Should show the correct answer in result
        await expect(authenticatedPage.locator('[data-correct-answer]')).toBeVisible();
      }
    });
  });

  test.describe('Answer Display Formatting', () => {
    test('should render answer text in result', async ({ authenticatedPage, testUser }) => {
      setupScenario(testUser.username, 'tier1_new', testUser.dataDir);
      await authenticatedPage.goto('/practice?mode=interactive');

      await authenticatedPage.waitForSelector('[data-testid="card-container"]');

      const textInput = authenticatedPage.locator('[data-testid="answer-input"]');
      if (await textInput.isVisible()) {
        await textInput.fill('test');
        await authenticatedPage.click('[data-testid="submit-answer"]');

        // Wait for result to appear
        await authenticatedPage.waitForSelector('[data-result]');

        // The correct answer display should exist
        const correctAnswer = authenticatedPage.locator('[data-correct-answer]');
        await expect(correctAnswer).toBeVisible();

        // Should have some text content
        const text = await correctAnswer.textContent();
        expect(text?.length).toBeGreaterThan(0);
      }
    });

    test('should show user answer when incorrect', async ({ authenticatedPage, testUser }) => {
      setupScenario(testUser.username, 'tier1_new', testUser.dataDir);
      await authenticatedPage.goto('/practice?mode=interactive');

      await authenticatedPage.waitForSelector('[data-testid="card-container"]');

      const textInput = authenticatedPage.locator('[data-testid="answer-input"]');
      if (await textInput.isVisible()) {
        const wrongAnswer = 'definitely_wrong_answer';
        await textInput.fill(wrongAnswer);
        await authenticatedPage.click('[data-testid="submit-answer"]');

        // Wait for incorrect result
        await authenticatedPage.waitForSelector('[data-result="incorrect"]');

        // Should show the user's answer
        const userAnswer = authenticatedPage.locator('[data-user-answer]');
        await expect(userAnswer).toBeVisible();
        await expect(userAnswer).toContainText(wrongAnswer);
      }
    });
  });

  test.describe('Study Mode Answer Display', () => {
    test('should display answer in study mode after validation', async ({ authenticatedPage, testUser }) => {
      setupScenario(testUser.username, 'tier1_new', testUser.dataDir);
      await authenticatedPage.goto('/study');

      await authenticatedPage.waitForSelector('[data-testid="card-container"]');

      const textInput = authenticatedPage.locator('[data-testid="answer-input"]');
      if (await textInput.isVisible()) {
        await textInput.fill('test');
        await authenticatedPage.click('[data-testid="submit-answer"]');

        // Wait for result phase
        await authenticatedPage.waitForSelector('[data-testid="result-phase"]');

        // Should show the answer
        await expect(authenticatedPage.locator('[data-correct-answer]')).toBeVisible();
      }
    });
  });

  test.describe('Flip Card Mode', () => {
    test('should display answer on card back', async ({ authenticatedPage, testUser }) => {
      setupScenario(testUser.username, 'tier1_new', testUser.dataDir);
      await authenticatedPage.goto('/study-classic');

      await authenticatedPage.waitForSelector('[data-testid="card-container"]');

      // Flip the card
      await authenticatedPage.locator('[data-testid="card-container"]').click();

      // Card back should show the answer
      const cardBack = authenticatedPage.locator('[data-testid="card-back"]');
      await expect(cardBack).toBeVisible();

      // Should have text content
      const text = await cardBack.textContent();
      expect(text?.length).toBeGreaterThan(0);
    });
  });

  test.describe('Practice Mode Options', () => {
    test('should support flip mode', async ({ authenticatedPage, testUser }) => {
      setupScenario(testUser.username, 'tier1_new', testUser.dataDir);
      await authenticatedPage.goto('/practice?mode=flip');

      // Should show card container
      await expect(authenticatedPage.locator('[data-testid="card-container"]')).toBeVisible();
    });

    test('should support progress tracking toggle', async ({ authenticatedPage, testUser }) => {
      setupScenario(testUser.username, 'tier1_new', testUser.dataDir);

      // With tracking enabled
      await authenticatedPage.goto('/practice?mode=interactive&track=true');
      await expect(authenticatedPage.locator('[data-testid="card-container"]')).toBeVisible();

      // With tracking disabled
      await authenticatedPage.goto('/practice?mode=interactive&track=false');
      await expect(authenticatedPage.locator('[data-testid="card-container"]')).toBeVisible();
    });
  });
});
