import { test as base, Page, expect, TestInfo } from '@playwright/test';
import { execSync } from 'child_process';
import * as crypto from 'crypto';
import * as path from 'path';

// Project root and paths
const PROJECT_ROOT = path.resolve(__dirname, '../../..');
const PY_SCRIPTS_DIR = path.join(PROJECT_ROOT, 'py_scripts');

// Test user configuration
export interface TestUser {
  username: string;
  password: string;
  passwordHash: string;
  dataDir: string;
}

// Compute client-side password hash (SHA-256 of password:username)
function computePasswordHash(password: string, username: string): string {
  const combined = `${password}:${username}`;
  return crypto.createHash('sha256').update(combined).digest('hex');
}

// Get the data directory for the current test project
function getDataDir(testInfo: TestInfo): string {
  const metadata = testInfo.project.metadata as { dataDir?: string } | undefined;
  if (!metadata?.dataDir) {
    // Fallback to default
    return path.join(PROJECT_ROOT, 'data');
  }

  return path.isAbsolute(metadata.dataDir)
    ? metadata.dataDir
    : path.join(PROJECT_ROOT, metadata.dataDir);
}

// Create a test user via db-manager CLI (environment-aware)
export function createTestUser(
  username: string,
  password: string = 'test123',
  dataDir?: string
): TestUser {
  const effectiveDataDir = dataDir || path.join(PROJECT_ROOT, 'data');
  const dataDirArg = dataDir ? ` --data-dir "${dataDir}"` : '';
  const cmd = `uv run db-manager create-test-user ${username} --password ${password}${dataDirArg}`;
  console.log(`[createTestUser] Running: ${cmd}`);
  console.log(`[createTestUser] cwd: ${PY_SCRIPTS_DIR}`);
  try {
    const result = execSync(cmd, {
      cwd: PY_SCRIPTS_DIR,
      stdio: 'pipe',
      encoding: 'utf-8',
    });
    console.log(`[createTestUser] Success: ${result.trim()}`);
  } catch (e) {
    const error = e as { stderr?: Buffer; stdout?: Buffer; message?: string };
    const stderr = error.stderr?.toString() || '';
    const stdout = error.stdout?.toString() || '';
    console.error(`[createTestUser] FAILED for ${username}:`);
    console.error(`  dataDir: ${dataDir}`);
    console.error(`  stderr: ${stderr}`);
    console.error(`  stdout: ${stdout}`);
    throw new Error(`Failed to create test user ${username}: ${stderr || error.message}`);
  }
  return {
    username,
    password,
    passwordHash: computePasswordHash(password, username),
    dataDir: effectiveDataDir,
  };
}

// Delete a test user (environment-aware)
export function deleteTestUser(username: string, dataDir?: string): void {
  try {
    const dataDirArg = dataDir ? ` --data-dir "${dataDir}"` : '';
    execSync(
      `uv run db-manager delete-user ${username} --yes${dataDirArg}`,
      {
        cwd: PY_SCRIPTS_DIR,
        stdio: 'pipe',
      }
    );
  } catch (e) {
    // Ignore errors (user might not exist)
  }
}

// Apply scenario preset via db-manager CLI (environment-aware)
export function setupScenario(username: string, scenario: string, dataDir?: string): void {
  const dataDirArg = dataDir ? ` --data-dir "${dataDir}"` : '';
  try {
    execSync(
      `uv run db-manager apply-preset ${scenario} --user ${username}${dataDirArg}`,
      {
        cwd: PY_SCRIPTS_DIR,
        stdio: 'pipe',
      }
    );
  } catch (e) {
    const error = e as { stderr?: Buffer };
    const stderr = error.stderr?.toString() || '';
    throw new Error(`Failed to setup scenario ${scenario} for ${username}: ${stderr}`);
  }
}

// Login helper function
export async function login(page: Page, user: TestUser): Promise<void> {
  await page.goto('/login', { waitUntil: 'networkidle' });
  await page.fill('[data-testid="username-input"]', user.username);
  await page.fill('[data-testid="password-input"]', user.password);

  await Promise.all([
    page.waitForURL('/'),
    page.click('[data-testid="login-submit"]'),
  ]);
}

// Extended test fixture with authentication
export const test = base.extend<{
  testUser: TestUser;
  authenticatedPage: Page;
  dataDir: string;
}>({
  // Provide the data directory for the current project
  dataDir: async ({}, use, testInfo) => {
    await use(getDataDir(testInfo));
  },

  // Create a unique test user for each test
  testUser: async ({ dataDir }, use) => {
    const username = `_test_e2e_${Date.now()}`;
    const user = createTestUser(username, 'test123', dataDir);
    await use(user);
    deleteTestUser(username, dataDir);
  },

  // Provide an authenticated page
  authenticatedPage: async ({ page, testUser }, use) => {
    await login(page, testUser);
    await use(page);
  },
});

export { expect };
