import { test, expect, Page } from '../fixtures/auth';

/**
 * Exercise Flow E2E Tests
 *
 * Uses test_exercises_pack fixture with 6 "be verb" cloze exercises:
 * - Exercise 1: "I ___ happy" → "am"
 * - Exercise 2: "She ___ a student" → "is"
 * - Exercise 3: "They ___ friends" → "are"
 * - Exercise 4: "We ___ learning" → "are"
 * - Exercise 5: "It ___ cold" → "is"
 * - Exercise 6: "I ___ tired" → "am"
 */

/** Navigate to the first lesson in test_exercises_pack */
async function goToLesson(page: Page): Promise<void> {
  await page.goto('/exercises');
  await page.click('a[href^="/exercises/pack/test_exercises_pack"]');
  await page.click('a[href*="/lesson/"]');
  await expect(page.locator('[data-testid="choice-grid"]')).toBeVisible();
}

/** Select an answer by its text (e.g., "am", "is", "are") */
async function selectAnswer(page: Page, answer: string): Promise<void> {
  await page.locator(`[data-testid="choice-option"]:has-text("${answer}")`).click();
}

/** Submit the selected answer */
async function submitAnswer(page: Page): Promise<void> {
  await page.click('[data-testid="submit-answer"]');
  await expect(page.locator('[data-testid="result-phase"]')).toBeVisible();
}

/** Click next to advance to the next exercise */
async function goToNextExercise(page: Page): Promise<void> {
  await page.click('[data-testid="next-exercise"]');
  // Wait for either the next exercise or completion screen
  await expect(
    page.locator('[data-testid="choice-grid"], [data-testid="lesson-complete"]').first()
  ).toBeVisible();
}

test.describe('Exercise Flow', () => {
  // Run serially to avoid race conditions with pack setup
  test.describe.configure({ mode: 'serial' });

  // Setup pack before each test
  test.beforeEach(async ({ adminPage }) => {
    await adminPage.goto('/settings');

    const testPackCard = adminPage.locator('#pack-card-test_exercises_pack');
    const packExists = await testPackCard.isVisible().catch(() => false);
    if (!packExists) {
      test.skip(true, 'test_exercises_pack not found');
      return;
    }

    // Enable pack if needed
    const isEnabled = await testPackCard.evaluate((el) =>
      el.classList.contains('bg-green-50') || el.className.includes('bg-green-')
    );
    if (!isEnabled) {
      await testPackCard.locator('button:has-text("Enable")').click();
      await expect(testPackCard.locator('button:has-text("Disable")')).toBeVisible({ timeout: 15000 });
    }

    // Make public if needed
    adminPage.on('dialog', (dialog) => dialog.accept());
    const manageAccess = testPackCard.locator('summary:has-text("Manage access")');
    if (await manageAccess.isVisible()) {
      await manageAccess.click();
      await adminPage.waitForTimeout(300);
    }
    const makePublicBtn = adminPage.locator('#pack-permissions-test_exercises_pack button:has-text("Make Public")');
    if (await makePublicBtn.isVisible()) {
      await makePublicBtn.click();
      await adminPage.waitForTimeout(500);
    }
  });

  test.describe('Navigation', () => {
    test('can navigate from home to exercise session', async ({ authenticatedPage }) => {
      // Start from home
      await authenticatedPage.goto('/');

      // Find and click exercises link in navbar
      await authenticatedPage.click('a[href="/exercises"]');
      await expect(authenticatedPage).toHaveURL('/exercises');

      // Should see the test pack
      const packLink = authenticatedPage.locator('a[href^="/exercises/pack/test_exercises_pack"]');
      await expect(packLink).toBeVisible();
      await packLink.click();

      // Should see lessons
      const lessonLink = authenticatedPage.locator('a[href*="/lesson/"]');
      await expect(lessonLink).toBeVisible();
      await lessonLink.click();

      // Should be in exercise session with all UI elements
      await expect(authenticatedPage.locator('[data-testid="exercise-sentence"]')).toBeVisible();
      await expect(authenticatedPage.locator('[data-testid="choice-grid"]')).toBeVisible();
      await expect(authenticatedPage.locator('[data-testid="progress-bar"]')).toContainText('Progress: 1 / 6');
    });
  });

  test.describe('Exercise Completion', () => {
    test('can complete single exercise with correct answer', async ({ authenticatedPage }) => {
      await goToLesson(authenticatedPage);

      // First exercise: "I ___ happy" → correct answer is "am"
      await expect(authenticatedPage.locator('[data-testid="exercise-sentence"]')).toContainText('I');
      await expect(authenticatedPage.locator('[data-testid="exercise-sentence"]')).toContainText('happy');

      // Select correct answer
      await selectAnswer(authenticatedPage, 'am');

      // Submit button should be enabled
      await expect(authenticatedPage.locator('[data-testid="submit-answer"]')).toBeEnabled();

      // Submit and verify correct feedback
      await submitAnswer(authenticatedPage);
      await expect(authenticatedPage.locator('[data-testid="result-correct"]')).toBeVisible();
      await expect(authenticatedPage.locator('[data-testid="correct-answer"]')).toContainText('am');

      // Next button should be visible
      await expect(authenticatedPage.locator('[data-testid="next-exercise"]')).toBeVisible();
    });

    test('shows incorrect feedback with user answer when wrong', async ({ authenticatedPage }) => {
      await goToLesson(authenticatedPage);

      // First exercise: "I ___ happy" → select wrong answer "is"
      await selectAnswer(authenticatedPage, 'is');
      await submitAnswer(authenticatedPage);

      // Should show incorrect feedback
      await expect(authenticatedPage.locator('[data-testid="result-incorrect"]')).toBeVisible();

      // Should show what user answered
      await expect(authenticatedPage.locator('[data-testid="user-answer"]')).toContainText('is');

      // Should show correct answer
      await expect(authenticatedPage.locator('[data-testid="correct-answer"]')).toContainText('am');
    });
  });

  test.describe('Progress Tracking', () => {
    test('progress bar updates after completing each exercise', async ({ authenticatedPage }) => {
      await goToLesson(authenticatedPage);

      // Verify starting at 1/6
      await expect(authenticatedPage.locator('[data-testid="progress-bar"]')).toContainText('1 / 6');

      // Complete exercise 1 (correct answer: "am")
      await selectAnswer(authenticatedPage, 'am');
      await submitAnswer(authenticatedPage);
      await goToNextExercise(authenticatedPage);

      // Should be at 2/6
      await expect(authenticatedPage.locator('[data-testid="progress-bar"]')).toContainText('2 / 6');

      // Complete exercise 2 (correct answer: "is")
      await selectAnswer(authenticatedPage, 'is');
      await submitAnswer(authenticatedPage);
      await goToNextExercise(authenticatedPage);

      // Should be at 3/6
      await expect(authenticatedPage.locator('[data-testid="progress-bar"]')).toContainText('3 / 6');
    });

    test('can complete entire lesson and see completion screen', async ({ authenticatedPage }) => {
      await goToLesson(authenticatedPage);

      // Complete all 6 exercises with correct answers
      const answers = ['am', 'is', 'are', 'are', 'is', 'am'];

      for (let i = 0; i < answers.length; i++) {
        await selectAnswer(authenticatedPage, answers[i]);
        await submitAnswer(authenticatedPage);
        await goToNextExercise(authenticatedPage);
      }

      // Should show completion screen
      await expect(authenticatedPage.locator('[data-testid="lesson-complete"]')).toBeVisible();
      await expect(authenticatedPage.locator('[data-testid="progress-bar"]')).toContainText('Complete');
    });
  });

  test.describe('Keyboard Navigation', () => {
    test('can complete exercise using only keyboard', async ({ authenticatedPage }) => {
      await goToLesson(authenticatedPage);

      // First exercise: "I ___ happy" → "am" should be one of the choices (1-4)
      // Find which number key corresponds to "am"
      const choices = authenticatedPage.locator('[data-testid="choice-option"]');
      const count = await choices.count();

      let amKeyNumber: string | null = null;
      for (let i = 0; i < count; i++) {
        const text = await choices.nth(i).textContent();
        if (text?.includes('am')) {
          amKeyNumber = (i + 1).toString();
          break;
        }
      }
      expect(amKeyNumber).not.toBeNull();

      // Press number key to select
      await authenticatedPage.keyboard.press(amKeyNumber!);

      // Submit button should be enabled
      await expect(authenticatedPage.locator('[data-testid="submit-answer"]')).toBeEnabled();

      // Press Enter to submit
      await authenticatedPage.keyboard.press('Enter');

      // Should show result
      await expect(authenticatedPage.locator('[data-testid="result-phase"]')).toBeVisible();

      // Press Enter again to go to next
      await authenticatedPage.keyboard.press('Enter');

      // Should be on exercise 2
      await expect(authenticatedPage.locator('[data-testid="progress-bar"]')).toContainText('2 / 6');
    });
  });

  test.describe('Back Navigation', () => {
    test('can navigate back to lesson list from exercise', async ({ authenticatedPage }) => {
      await goToLesson(authenticatedPage);

      // Click back link
      await authenticatedPage.click('a:has-text("Back to")');

      // Should be on pack page with lesson list
      await expect(authenticatedPage.locator('a[href*="/lesson/"]')).toBeVisible();
    });

    test('can navigate back to pack list from lesson list', async ({ authenticatedPage }) => {
      await authenticatedPage.goto('/exercises');
      await authenticatedPage.click('a[href^="/exercises/pack/test_exercises_pack"]');

      // Click back link (shows "← All Exercises")
      await authenticatedPage.click('a[href="/exercises"]');

      // Should be on exercises page with pack list
      await expect(authenticatedPage).toHaveURL('/exercises');
      await expect(authenticatedPage.locator('a[href^="/exercises/pack/"]')).toBeVisible();
    });
  });
});
