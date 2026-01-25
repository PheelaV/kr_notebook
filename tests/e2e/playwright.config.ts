import { defineConfig, devices, Project } from '@playwright/test';

/**
 * Playwright configuration with project-based test isolation and multi-browser support.
 *
 * Each test suite runs in its own isolated environment:
 * - Separate DATA_DIR (database isolation)
 * - Separate PORT (no conflicts)
 * - Tests within a suite share the environment
 * - Tests across suites are fully isolated
 *
 * Browser targeting:
 * - Default: runs on Chrome, Firefox, and WebKit (all 3 browsers)
 * - Dev mode: use BROWSER=chrome|firefox|webkit to target single browser
 *
 * Examples:
 *   npm test                              # All suites, all browsers
 *   BROWSER=chrome npm test               # All suites, Chrome only
 *   npm test -- --project=auth-chrome     # Single suite + browser
 *   npm test -- --project=fresh-install-firefox  # Single suite + browser
 */

// Browser configurations
const BROWSERS = {
  chrome: { name: 'chrome', device: devices['Desktop Chrome'] },
  firefox: { name: 'firefox', device: devices['Desktop Firefox'] },
  webkit: { name: 'webkit', device: devices['Desktop Safari'] },
} as const;

type BrowserName = keyof typeof BROWSERS;

// Determine which browsers to run based on BROWSER/SKIP_WEBKIT env vars
function getTargetBrowsers(): BrowserName[] {
  const browserEnv = process.env.BROWSER?.toLowerCase();
  const skipWebkit = process.env.SKIP_WEBKIT === '1';

  if (browserEnv && browserEnv in BROWSERS) {
    return [browserEnv as BrowserName];
  }

  // Default: all browsers (unless SKIP_WEBKIT is set)
  if (skipWebkit) {
    return ['chrome', 'firefox'];
  }
  return ['chrome', 'firefox', 'webkit'];
}

// Test suite definitions (port and dataDir are computed per browser)
interface TestSuite {
  name: string;
  testMatch: string;
  freshInstall?: boolean;
  testAdminPassword?: string;
}

const TEST_SUITES: TestSuite[] = [
  { name: 'auth', testMatch: 'auth.spec.ts' },
  { name: 'study', testMatch: 'study.spec.ts' },
  { name: 'registration', testMatch: 'registration.spec.ts' },
  { name: 'admin', testMatch: 'admin.spec.ts' },
  { name: 'groups', testMatch: 'groups.spec.ts' },
  { name: 'pack-permissions', testMatch: 'pack-permissions.spec.ts' },
  { name: 'settings', testMatch: 'settings.spec.ts' },
  { name: 'menu-visibility', testMatch: 'menu-visibility.spec.ts' },
  { name: 'navbar-dropdown', testMatch: 'navbar-dropdown.spec.ts' },
  { name: 'offline-study', testMatch: 'offline-study.spec.ts' },
  { name: 'offline-sync', testMatch: 'offline-sync.spec.ts' },
  { name: 'lesson-filtering', testMatch: 'lesson-filtering.spec.ts' },
  { name: 'vocabulary-search', testMatch: 'vocabulary-search.spec.ts' },
  {
    name: 'fresh-install',
    testMatch: 'fresh-install.spec.ts',
    freshInstall: true,
    testAdminPassword: 'e2e_test_admin_pwd',
  },
];

// Base port for test servers (each suite+browser gets unique port)
const BASE_PORT = 3001;

// Calculate unique port for suite+browser combination
// Layout: suite0-chrome=3001, suite0-firefox=3002, suite0-webkit=3003,
//         suite1-chrome=3004, suite1-firefox=3005, suite1-webkit=3006, ...
function getPort(suiteIndex: number, browserIndex: number): number {
  return BASE_PORT + suiteIndex * 3 + browserIndex;
}

// Get data directory for suite+browser combination
function getDataDir(suiteName: string, browserName: string): string {
  return `data/test/e2e-${suiteName}-${browserName}`;
}

// Generate projects: suite × browser combinations (each gets unique port/dataDir)
function generateProjects(): Project[] {
  const targetBrowsers = getTargetBrowsers();
  const browserList = Object.keys(BROWSERS) as BrowserName[];
  const projects: Project[] = [];

  for (let suiteIndex = 0; suiteIndex < TEST_SUITES.length; suiteIndex++) {
    const suite = TEST_SUITES[suiteIndex];

    for (const browserName of targetBrowsers) {
      const browser = BROWSERS[browserName];
      const browserIndex = browserList.indexOf(browserName);
      const port = getPort(suiteIndex, browserIndex);
      const dataDir = getDataDir(suite.name, browser.name);

      projects.push({
        name: `${suite.name}-${browser.name}`,
        testMatch: suite.testMatch,
        use: {
          ...browser.device,
          baseURL: `http://localhost:${port}`,
        },
        metadata: {
          dataDir,
          port,
          freshInstall: suite.freshInstall,
          testAdminPassword: suite.testAdminPassword,
        },
      });
    }
  }

  return projects;
}

export default defineConfig({
  testDir: './specs',
  fullyParallel: true,
  forbidOnly: !!process.env.CI,
  retries: 2, // Retry flaky tests up to 2 times (3 total attempts)
  workers: process.env.CI ? 1 : undefined,
  reporter: 'html',

  // Global setup/teardown for server management
  globalSetup: require.resolve('./global-setup'),
  globalTeardown: require.resolve('./global-teardown'),

  use: {
    trace: 'on-first-retry',
    screenshot: 'only-on-failure',
  },

  projects: generateProjects(),
});
