import { test, expect } from '../fixtures/auth';

test.describe('Navbar Library Dropdown Consistency', () => {
  test('Library link behavior is consistent across pages', async ({ authenticatedPage }) => {
    // First, check the Library element on the home page
    await authenticatedPage.goto('/');

    // Get the Library link/dropdown structure on home page
    const homeLibraryLink = authenticatedPage.locator('nav a:has-text("Library")').first();
    await expect(homeLibraryLink).toBeVisible();

    // Check if there's a dropdown chevron (indicates dropdown mode)
    const homeHasChevron = await authenticatedPage
      .locator('nav .relative.group a:has-text("Library") iconify-icon[icon*="chevron"]')
      .count() > 0;

    // Now check reference page - should have same behavior
    await authenticatedPage.goto('/reference');
    const refLibraryLink = authenticatedPage.locator('nav a:has-text("Library")').first();
    await expect(refLibraryLink).toBeVisible();

    const refHasChevron = await authenticatedPage
      .locator('nav .relative.group a:has-text("Library") iconify-icon[icon*="chevron"]')
      .count() > 0;

    // The key assertion: Library should behave the same on both pages
    expect(refHasChevron).toBe(homeHasChevron);
  });

  test('Library link behavior consistent on study, progress, reference, and library pages', async ({ authenticatedPage }) => {
    const pages = ['/', '/study', '/progress', '/reference', '/library'];
    const results: { page: string; hasChevron: boolean }[] = [];

    for (const pagePath of pages) {
      await authenticatedPage.goto(pagePath);

      // Wait for nav to be visible
      await expect(authenticatedPage.locator('nav')).toBeVisible();

      const hasChevron = await authenticatedPage
        .locator('nav .relative.group a:has-text("Library") iconify-icon[icon*="chevron"]')
        .count() > 0;

      results.push({ page: pagePath, hasChevron });
    }

    // All pages should have the same chevron state
    const firstState = results[0].hasChevron;
    for (const result of results) {
      expect(result.hasChevron,
        `Library dropdown state on ${result.page} should match home page`
      ).toBe(firstState);
    }
  });

  test('Library dropdown expands on hover when present', async ({ authenticatedPage }) => {
    await authenticatedPage.goto('/');

    // Check if this user has the dropdown (has chevron)
    const hasDropdown = await authenticatedPage
      .locator('nav .relative.group a:has-text("Library") iconify-icon[icon*="chevron"]')
      .count() > 0;

    if (hasDropdown) {
      // Find the dropdown container and hover
      const dropdownContainer = authenticatedPage.locator('nav .relative.group').filter({
        has: authenticatedPage.locator('a:has-text("Library")')
      }).first();

      await dropdownContainer.hover();

      // The dropdown menu should become visible
      const dropdownMenu = dropdownContainer.locator('.absolute');
      await expect(dropdownMenu).toBeVisible({ timeout: 2000 });

      // Should see Characters and Vocabulary links
      await expect(dropdownMenu.getByText('Characters')).toBeVisible();
      await expect(dropdownMenu.getByText('Vocabulary')).toBeVisible();
    } else {
      // No dropdown - just verify Library link works
      const libraryLink = authenticatedPage.locator('nav a:has-text("Library")').first();
      await expect(libraryLink).toHaveAttribute('href', '/library');
    }
  });
});
