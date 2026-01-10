/**
 * Playwright global teardown: Stop servers and clean up test environments.
 *
 * For each project that was set up:
 * 1. Stop the server process
 * 2. Optionally clean up the test environment
 *
 * Uses server info saved by global-setup in .servers/
 */

import { FullConfig } from '@playwright/test';
import * as fs from 'fs';
import * as path from 'path';

const SERVERS_DIR = path.join(__dirname, '.servers');

interface ServerInfo {
  pid: number;
  port: number;
  dataDir: string;
}

// Read server info from file
function readServerInfo(projectName: string): ServerInfo | null {
  const infoPath = path.join(SERVERS_DIR, `${projectName}.json`);
  if (!fs.existsSync(infoPath)) {
    return null;
  }
  return JSON.parse(fs.readFileSync(infoPath, 'utf-8'));
}

// Stop a server by PID
function stopServer(info: ServerInfo): void {
  try {
    // Kill the process group (negative PID kills the group)
    process.kill(-info.pid, 'SIGTERM');
  } catch (e) {
    // Try killing just the process
    try {
      process.kill(info.pid, 'SIGTERM');
    } catch {
      // Process might already be dead
    }
  }
}

// Clean up test environment
function cleanupEnv(dataDir: string): void {
  if (fs.existsSync(dataDir)) {
    fs.rmSync(dataDir, { recursive: true });
  }
}

async function globalTeardown(config: FullConfig): Promise<void> {
  console.log('\n=== Playwright Global Teardown ===\n');

  // Read preserve flag from environment
  const preserveEnv = process.env.PRESERVE_TEST_ENV === '1';

  if (!fs.existsSync(SERVERS_DIR)) {
    console.log('No servers directory found, nothing to clean up.');
    return;
  }

  const serverFiles = fs.readdirSync(SERVERS_DIR).filter((f) => f.endsWith('.json'));

  for (const file of serverFiles) {
    const projectName = path.basename(file, '.json');
    const info = readServerInfo(projectName);

    if (!info) {
      continue;
    }

    console.log(`Cleaning up project: ${projectName}`);

    // 1. Stop server
    console.log(`  Stopping server (PID ${info.pid})...`);
    stopServer(info);

    // 2. Clean up test environment (unless preserved)
    if (!preserveEnv) {
      console.log(`  Removing test environment: ${info.dataDir}`);
      cleanupEnv(info.dataDir);
    } else {
      console.log(`  Preserving test environment: ${info.dataDir}`);
    }

    // 3. Remove server info file
    fs.unlinkSync(path.join(SERVERS_DIR, file));
  }

  // Remove servers directory if empty
  try {
    fs.rmdirSync(SERVERS_DIR);
  } catch {
    // Not empty or doesn't exist
  }

  console.log('\n=== Teardown Complete ===\n');
}

export default globalTeardown;
