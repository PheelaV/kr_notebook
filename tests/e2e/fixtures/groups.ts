/**
 * Group management helpers for E2E tests.
 *
 * These helpers use db-manager CLI commands to manage groups
 * in the test environment.
 */

import { execSync } from 'child_process';
import * as path from 'path';

// Project root and paths
const PROJECT_ROOT = path.resolve(__dirname, '../../..');
const PY_SCRIPTS_DIR = path.join(PROJECT_ROOT, 'py_scripts');

/**
 * Create a test group.
 */
export function createTestGroup(
  groupId: string,
  name: string,
  dataDir: string,
  description?: string
): void {
  const dataDirArg = ` --data-dir "${dataDir}"`;
  const descArg = description ? ` --description "${description}"` : '';
  const cmd = `uv run db-manager create-group ${groupId} "${name}"${descArg}${dataDirArg}`;

  try {
    execSync(cmd, {
      cwd: PY_SCRIPTS_DIR,
      stdio: 'pipe',
    });
  } catch (e) {
    const error = e as { stderr?: Buffer };
    const stderr = error.stderr?.toString() || '';
    throw new Error(`Failed to create group ${groupId}: ${stderr}`);
  }
}

/**
 * Delete a test group.
 */
export function deleteTestGroup(groupId: string, dataDir: string): void {
  const cmd = `uv run db-manager delete-group ${groupId} --yes --data-dir "${dataDir}"`;

  try {
    execSync(cmd, {
      cwd: PY_SCRIPTS_DIR,
      stdio: 'pipe',
    });
  } catch (e) {
    // Ignore errors (group might not exist)
  }
}

/**
 * Add a user to a group.
 */
export function addUserToGroup(
  username: string,
  groupId: string,
  dataDir: string
): void {
  const cmd = `uv run db-manager add-to-group ${username} ${groupId} --data-dir "${dataDir}"`;

  try {
    execSync(cmd, {
      cwd: PY_SCRIPTS_DIR,
      stdio: 'pipe',
    });
  } catch (e) {
    const error = e as { stderr?: Buffer };
    const stderr = error.stderr?.toString() || '';
    throw new Error(`Failed to add ${username} to group ${groupId}: ${stderr}`);
  }
}

/**
 * Remove a user from a group.
 */
export function removeUserFromGroup(
  username: string,
  groupId: string,
  dataDir: string
): void {
  const cmd = `uv run db-manager remove-from-group ${username} ${groupId} --data-dir "${dataDir}"`;

  try {
    execSync(cmd, {
      cwd: PY_SCRIPTS_DIR,
      stdio: 'pipe',
    });
  } catch (e) {
    const error = e as { stderr?: Buffer };
    const stderr = error.stderr?.toString() || '';
    throw new Error(`Failed to remove ${username} from group ${groupId}: ${stderr}`);
  }
}
