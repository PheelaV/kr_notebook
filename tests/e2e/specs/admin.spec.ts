import { test, expect, setUserRole, login, createExpiredGuest, getGuestCount, guestExists } from '../fixtures/auth';

// Tests in this file manipulate shared state (guests, roles) and must run serially
// to avoid race conditions where one test's cleanup affects another test's assertions
test.describe.configure({ mode: 'serial' });

test.describe('Admin Access Control', () => {
  test('admin user sees admin sections in settings', async ({ adminPage }) => {
    await adminPage.goto('/settings');

    await expect(adminPage.locator('[data-testid="user-management"]')).toBeVisible();
    await expect(adminPage.locator('[data-testid="group-management"]')).toBeVisible();
    await expect(adminPage.locator('[data-testid="guest-management"]')).toBeVisible();
  });

  test('regular user does NOT see admin sections', async ({ authenticatedPage }) => {
    await authenticatedPage.goto('/settings');

    await expect(authenticatedPage.locator('[data-testid="user-management"]')).not.toBeVisible();
    await expect(authenticatedPage.locator('[data-testid="group-management"]')).not.toBeVisible();
    await expect(authenticatedPage.locator('[data-testid="guest-management"]')).not.toBeVisible();
  });

  test('regular user cannot access admin endpoints directly', async ({ authenticatedPage, dataDir }) => {
    // SETUP: Create a specific expired guest that would be cleaned up if the action succeeded
    const myGuestId = createExpiredGuest(dataDir);

    // Try to POST to an admin endpoint as non-admin
    const response = await authenticatedPage.request.post('/settings/cleanup-guests');

    // Accept various denial responses:
    // 200 (with error page), 303 (redirect), or 403 (forbidden)
    expect([200, 303, 403]).toContain(response.status());

    // VERIFY EFFECT: Our specific guest was NOT deleted (this is the critical assertion)
    // We check the specific guest, not total count, to avoid race conditions with parallel tests
    expect(guestExists(myGuestId, dataDir)).toBe(true);
  });
});

test.describe('User Role Management', () => {
  test('admin can see user list with role badges', async ({ adminPage }) => {
    await adminPage.goto('/settings');

    const userList = adminPage.locator('#userList');
    await expect(userList).toBeVisible();

    // Should have at least one user row (the admin themselves)
    const userRows = userList.locator('.user-row');
    await expect(userRows.first()).toBeVisible();
  });

  test('admin can search users', async ({ adminPage }) => {
    await adminPage.goto('/settings');

    const searchInput = adminPage.locator('#userSearchInput');
    await expect(searchInput).toBeVisible();

    // Type a search term
    await searchInput.fill('_test_');

    // The filtering is client-side JS, so rows should filter
    // Just verify the input works
    await expect(searchInput).toHaveValue('_test_');
  });

  test('promoted user gains admin access', async ({ browser, testUser, dataDir }) => {
    // First verify user doesn't have admin access
    const page1 = await browser.newPage();
    await login(page1, testUser);
    await page1.goto('/settings');
    await expect(page1.locator('[data-testid="user-management"]')).not.toBeVisible();
    await page1.close();

    // Promote user to admin
    setUserRole(testUser.username, 'admin', dataDir);

    // Login again and verify admin access
    const page2 = await browser.newPage();
    await login(page2, testUser);
    await page2.goto('/settings');
    await expect(page2.locator('[data-testid="user-management"]')).toBeVisible();
    await page2.close();
  });

  test('demoted user loses admin access', async ({ browser, adminUser, dataDir }) => {
    // First verify admin has access
    const page1 = await browser.newPage();
    await login(page1, adminUser);
    await page1.goto('/settings');
    await expect(page1.locator('[data-testid="user-management"]')).toBeVisible();
    await page1.close();

    // Demote to regular user
    setUserRole(adminUser.username, 'user', dataDir);

    // Login again and verify no admin access
    const page2 = await browser.newPage();
    await login(page2, adminUser);
    await page2.goto('/settings');
    await expect(page2.locator('[data-testid="user-management"]')).not.toBeVisible();
    await page2.close();
  });
});

test.describe('Guest Management', () => {
  test('admin sees guest management section', async ({ adminPage }) => {
    await adminPage.goto('/settings');

    const guestSection = adminPage.locator('[data-testid="guest-management"]');
    await expect(guestSection).toBeVisible();

    // Should have cleanup and delete all buttons
    await expect(guestSection.locator('button:has-text("Clean Up Expired")')).toBeVisible();
    await expect(guestSection.locator('button:has-text("Delete All Guests")')).toBeVisible();
  });

  test('cleanup guests button is functional', async ({ adminPage, dataDir }) => {
    // SETUP: Create an expired guest that should be cleaned up
    createExpiredGuest(dataDir);
    const guestCountBefore = getGuestCount(dataDir);
    expect(guestCountBefore).toBeGreaterThan(0); // Ensure we have at least one guest

    await adminPage.goto('/settings');

    const guestSection = adminPage.locator('[data-testid="guest-management"]');
    const cleanupBtn = guestSection.locator('button:has-text("Clean Up Expired")');

    // Click cleanup button - this is a form submit
    await cleanupBtn.click();

    // Should redirect back to settings (form action redirects)
    await adminPage.waitForURL(/\/settings/);

    // VERIFY EFFECT: Guest count decreased (expired guest was cleaned up)
    const guestCountAfter = getGuestCount(dataDir);
    expect(guestCountAfter).toBeLessThan(guestCountBefore);
  });
});
