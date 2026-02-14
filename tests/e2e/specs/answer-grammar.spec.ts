import { test, expect, setupScenario } from '../fixtures/auth';

test.describe('Answer Grammar Visual Indicators', () => {
  test.describe('Practice Mode', () => {
    test('should display practice cards', async ({ authenticatedPage, testUser }) => {
      setupScenario(testUser.username, 'tier1_new', testUser.dataDir);

      await authenticatedPage.goto('/practice?mode=interactive');

      await expect(authenticatedPage.locator('[data-testid="card-container"]')).toBeVisible();
    });

    test('should show correct result on right answer', async ({ authenticatedPage, testUser }) => {
      setupScenario(testUser.username, 'tier1_new', testUser.dataDir);
      await authenticatedPage.goto('/practice?mode=interactive');

      await expect(authenticatedPage.locator('[data-testid="card-container"]')).toBeVisible();

      const textInput = authenticatedPage.locator('[data-testid="answer-input"]');
      test.skip(!(await textInput.isVisible()), 'Card uses MCQ, not text input');

      const cardContainer = authenticatedPage.locator('[data-testid="card-container"]');
      const correctAnswer = await cardContainer.getAttribute('data-answer');

      if (correctAnswer) {
        await textInput.fill(correctAnswer);
        await authenticatedPage.locator('[data-testid="submit-answer"]').click();

        await expect(authenticatedPage.locator('[data-result="correct"]')).toBeVisible();
      }
    });

    test('should show incorrect result on wrong answer', async ({ authenticatedPage, testUser }) => {
      setupScenario(testUser.username, 'tier1_new', testUser.dataDir);
      await authenticatedPage.goto('/practice?mode=interactive');

      await expect(authenticatedPage.locator('[data-testid="card-container"]')).toBeVisible();

      const textInput = authenticatedPage.locator('[data-testid="answer-input"]');
      test.skip(!(await textInput.isVisible()), 'Card uses MCQ, not text input');

      await textInput.fill('xyzzy_wrong_answer_12345');
      await authenticatedPage.locator('[data-testid="submit-answer"]').click();

      await expect(authenticatedPage.locator('[data-result="incorrect"]')).toBeVisible();
    });

    test('should display correct answer after validation', async ({ authenticatedPage, testUser }) => {
      setupScenario(testUser.username, 'tier1_new', testUser.dataDir);
      await authenticatedPage.goto('/practice?mode=interactive');

      await expect(authenticatedPage.locator('[data-testid="card-container"]')).toBeVisible();

      const textInput = authenticatedPage.locator('[data-testid="answer-input"]');
      test.skip(!(await textInput.isVisible()), 'Card uses MCQ, not text input');

      await textInput.fill('test');
      await authenticatedPage.locator('[data-testid="submit-answer"]').click();

      await expect(authenticatedPage.locator('[data-correct-answer]')).toBeVisible();
    });
  });

  test.describe('Answer Display Formatting', () => {
    test('should render answer text in result', async ({ authenticatedPage, testUser }) => {
      setupScenario(testUser.username, 'tier1_new', testUser.dataDir);
      await authenticatedPage.goto('/practice?mode=interactive');

      await expect(authenticatedPage.locator('[data-testid="card-container"]')).toBeVisible();

      const textInput = authenticatedPage.locator('[data-testid="answer-input"]');
      test.skip(!(await textInput.isVisible()), 'Card uses MCQ, not text input');

      await textInput.fill('test');
      await authenticatedPage.locator('[data-testid="submit-answer"]').click();

      await expect(authenticatedPage.locator('[data-result]')).toBeVisible();

      const correctAnswer = authenticatedPage.locator('[data-correct-answer]');
      await expect(correctAnswer).toBeVisible();

      const text = await correctAnswer.textContent();
      expect(text?.length).toBeGreaterThan(0);
    });

    test('should show user answer when incorrect', async ({ authenticatedPage, testUser }) => {
      setupScenario(testUser.username, 'tier1_new', testUser.dataDir);
      await authenticatedPage.goto('/practice?mode=interactive');

      await expect(authenticatedPage.locator('[data-testid="card-container"]')).toBeVisible();

      const textInput = authenticatedPage.locator('[data-testid="answer-input"]');
      test.skip(!(await textInput.isVisible()), 'Card uses MCQ, not text input');

      const wrongAnswer = 'definitely_wrong_answer';
      await textInput.fill(wrongAnswer);
      await authenticatedPage.locator('[data-testid="submit-answer"]').click();

      await expect(authenticatedPage.locator('[data-result="incorrect"]')).toBeVisible();

      const userAnswer = authenticatedPage.locator('[data-user-answer]');
      await expect(userAnswer).toBeVisible();
      await expect(userAnswer).toContainText(wrongAnswer);
    });
  });

  test.describe('Study Mode Answer Display', () => {
    test('should display answer in study mode after validation', async ({ authenticatedPage, testUser }) => {
      setupScenario(testUser.username, 'tier1_new', testUser.dataDir);
      await authenticatedPage.goto('/study');

      await expect(authenticatedPage.locator('[data-testid="card-container"]')).toBeVisible();

      const textInput = authenticatedPage.locator('[data-testid="answer-input"]');
      test.skip(!(await textInput.isVisible()), 'Card uses MCQ, not text input');

      await textInput.fill('test');
      await authenticatedPage.locator('[data-testid="submit-answer"]').click();

      await expect(authenticatedPage.locator('[data-testid="result-phase"]')).toBeVisible();

      await expect(authenticatedPage.locator('[data-correct-answer]')).toBeVisible();
    });
  });

  test.describe('Flip Card Mode', () => {
    test('should display answer on card back', async ({ authenticatedPage, testUser }) => {
      setupScenario(testUser.username, 'tier1_new', testUser.dataDir);
      await authenticatedPage.goto('/study-classic');

      await expect(authenticatedPage.locator('[data-testid="card-container"]')).toBeVisible();

      await authenticatedPage.locator('[data-testid="card-container"]').click();

      const cardBack = authenticatedPage.locator('[data-testid="card-back"]');
      await expect(cardBack).toBeVisible();

      const text = await cardBack.textContent();
      expect(text?.length).toBeGreaterThan(0);
    });
  });

  test.describe('Practice Mode Options', () => {
    test('should support flip mode', async ({ authenticatedPage, testUser }) => {
      setupScenario(testUser.username, 'tier1_new', testUser.dataDir);
      await authenticatedPage.goto('/practice?mode=flip');

      await expect(authenticatedPage.locator('[data-testid="card-container"]')).toBeVisible();
    });

    test('should support progress tracking toggle', async ({ authenticatedPage, testUser }) => {
      setupScenario(testUser.username, 'tier1_new', testUser.dataDir);

      await authenticatedPage.goto('/practice?mode=interactive&track=true');
      await expect(authenticatedPage.locator('[data-testid="card-container"]')).toBeVisible();

      await authenticatedPage.goto('/practice?mode=interactive&track=false');
      await expect(authenticatedPage.locator('[data-testid="card-container"]')).toBeVisible();
    });
  });
});
