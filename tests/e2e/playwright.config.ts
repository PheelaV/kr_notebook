import { defineConfig, devices } from '@playwright/test';

/**
 * Playwright configuration with project-based test isolation.
 *
 * Each test group (auth, study) runs in its own isolated environment:
 * - Separate DATA_DIR (database isolation)
 * - Separate PORT (no conflicts)
 * - Tests within a group share the environment
 * - Tests across groups are fully isolated
 *
 * Global setup starts servers, global teardown cleans up.
 */

export default defineConfig({
  testDir: './specs',
  fullyParallel: true,
  forbidOnly: !!process.env.CI,
  retries: process.env.CI ? 2 : 0,
  workers: process.env.CI ? 1 : undefined,
  reporter: 'html',

  // Global setup/teardown for server management
  globalSetup: require.resolve('./global-setup'),
  globalTeardown: require.resolve('./global-teardown'),

  use: {
    trace: 'on-first-retry',
    screenshot: 'only-on-failure',
  },

  projects: [
    // ==================== Auth Tests ====================
    // Tests for login, registration, session management
    {
      name: 'auth-tests',
      testMatch: 'auth.spec.ts',
      use: {
        ...devices['Desktop Chrome'],
        baseURL: 'http://localhost:3001',
      },
      metadata: {
        dataDir: 'data/test/e2e-auth',
        port: 3001,
      },
    },

    // ==================== Study Tests ====================
    // Tests for study flow, SRS, card review
    {
      name: 'study-tests',
      testMatch: 'study.spec.ts',
      use: {
        ...devices['Desktop Chrome'],
        baseURL: 'http://localhost:3002',
      },
      metadata: {
        dataDir: 'data/test/e2e-study',
        port: 3002,
      },
    },

    // ==================== Cross-Browser (Optional) ====================
    // Run the same tests on different browsers (shares server with auth-tests)
    // Uncomment to enable cross-browser testing
    /*
    {
      name: 'auth-firefox',
      testMatch: 'auth.spec.ts',
      use: {
        ...devices['Desktop Firefox'],
        baseURL: 'http://localhost:3001',
      },
      // No metadata - reuses auth-tests server
    },
    {
      name: 'auth-webkit',
      testMatch: 'auth.spec.ts',
      use: {
        ...devices['Desktop Safari'],
        baseURL: 'http://localhost:3001',
      },
      // No metadata - reuses auth-tests server
    },
    */
  ],
});
