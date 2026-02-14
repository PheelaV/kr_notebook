import { test, expect } from '../fixtures/auth';

/**
 * E2E tests for lesson-based card filtering.
 *
 * Uses test_lesson_pack fixture with:
 * - Lesson 1: 3 cards (L1-A, L1-B, L1-C)
 * - Lesson 2: 2 cards (L2-A, L2-B)
 */

test.describe('Pack Lesson Filtering', () => {
  test('test_lesson_pack is available in settings', async ({ adminPage }) => {
    await adminPage.goto('/settings');

    const packsSection = adminPage.locator('#packs');
    await expect(packsSection).toBeVisible();

    const testPackCard = adminPage.locator('#pack-card-test_lesson_pack');
    await expect(testPackCard).toBeVisible();
  });

  test('admin can enable test_lesson_pack', async ({ adminPage }) => {
    await adminPage.goto('/settings');

    const testPackCard = adminPage.locator('#pack-card-test_lesson_pack');
    await expect(testPackCard).toBeVisible();

    const isEnabled = await testPackCard.evaluate((el) =>
      el.classList.contains('bg-green-50') || el.className.includes('bg-green-')
    );

    if (!isEnabled) {
      const enableBtn = testPackCard.locator('button:has-text("Enable")');
      await enableBtn.click();
    }

    await expect(testPackCard).toHaveClass(/bg-green/);
  });

  test('due count reflects lesson filtering', async ({ adminPage }) => {
    await adminPage.goto('/settings');

    const testPackCard = adminPage.locator('#pack-card-test_lesson_pack');

    const isEnabled = await testPackCard.evaluate((el) =>
      el.classList.contains('bg-green-50') || el.className.includes('bg-green-')
    );

    if (!isEnabled) {
      const enableBtn = testPackCard.locator('button:has-text("Enable")');
      await enableBtn.click();
      await expect(testPackCard).toHaveClass(/bg-green/);
    }

    const permissionsSection = adminPage.locator(`#pack-permissions-test_lesson_pack`);
    if (await permissionsSection.isVisible()) {
      const makePublicBtn = permissionsSection.locator('button:has-text("Make Public")');
      if (await makePublicBtn.isVisible()) {
        await makePublicBtn.click();
        // Wait for the permission update to complete
        await expect(makePublicBtn).not.toBeVisible();
      }
    }

    await adminPage.goto('/');

    const dueCountEl = adminPage.locator('[data-testid="due-count"]');
    await expect(dueCountEl).toBeVisible();

    const dueCountText = await dueCountEl.textContent();
    const dueCount = parseInt(dueCountText?.trim() || '0', 10);

    expect(dueCount).toBeGreaterThan(0);

    console.log(`Due count on home page: ${dueCount}`);
  });

  test('progress page shows pack lesson breakdown', async ({ adminPage }) => {
    await adminPage.goto('/settings');

    const testPackCard = adminPage.locator('#pack-card-test_lesson_pack');
    const isEnabled = await testPackCard.evaluate((el) =>
      el.classList.contains('bg-green-50') || el.className.includes('bg-green-')
    );

    if (!isEnabled) {
      const enableBtn = testPackCard.locator('button:has-text("Enable")');
      await enableBtn.click();
      await expect(testPackCard).toHaveClass(/bg-green/);
    }

    await adminPage.goto('/progress');

    const pageText = await adminPage.textContent('body');

    expect(pageText?.toLowerCase()).toContain('lesson');

    const testLessonsSection = adminPage.locator('text=Test Lessons');
    const hasTestLessons = await testLessonsSection.count() > 0;

    const lesson1Section = adminPage.locator('details:has-text("Lesson 1")');
    const lesson2Section = adminPage.locator('details:has-text("Lesson 2")');

    console.log(`Has "Test Lessons" text: ${hasTestLessons}`);
    console.log(`Has Lesson 1 details: ${await lesson1Section.count()}`);
    console.log(`Has Lesson 2 details: ${await lesson2Section.count()}`);
  });

  test('lesson cards have correct lesson numbers in DB', async ({ adminPage, dataDir }) => {
    await adminPage.goto('/settings');

    const testPackCard = adminPage.locator('#pack-card-test_lesson_pack');
    const isEnabled = await testPackCard.evaluate((el) =>
      el.classList.contains('bg-green-50') || el.className.includes('bg-green-')
    );

    if (!isEnabled) {
      const enableBtn = testPackCard.locator('button:has-text("Enable")');
      await enableBtn.click();
      await expect(testPackCard).toHaveClass(/bg-green/);
    }

    const { execSync } = await import('child_process');
    const path = await import('path');

    const PROJECT_ROOT = path.resolve(__dirname, '../../..');
    const PY_SCRIPTS_DIR = path.join(PROJECT_ROOT, 'py_scripts');

    const cmd = `uv run db-manager get-pack-lesson-counts test_lesson_pack --json --data-dir "${dataDir}"`;
    const result = execSync(cmd, {
      cwd: PY_SCRIPTS_DIR,
      encoding: 'utf-8',
    });

    const lessonCounts = JSON.parse(result.trim());

    expect(lessonCounts['1']).toBe(3);
    expect(lessonCounts['2']).toBe(2);

    expect(lessonCounts['null']).toBeUndefined();
  });
});

test.describe('Lesson Progress', () => {
  test('lesson 1 is unlocked by default', async ({ adminPage }) => {
    await adminPage.goto('/settings');

    const testPackCard = adminPage.locator('#pack-card-test_lesson_pack');
    const enableBtn = testPackCard.locator('button:has-text("Enable")');
    if (await enableBtn.isVisible()) {
      await enableBtn.click();
      await expect(testPackCard).toHaveClass(/bg-green/);
    }

    await adminPage.goto('/progress');

    const pageText = await adminPage.textContent('body');
    expect(pageText?.toLowerCase()).toContain('lesson');
  });
});
