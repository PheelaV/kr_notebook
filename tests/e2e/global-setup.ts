/**
 * Playwright global setup: Initialize test environments and start servers.
 *
 * For each project with metadata.dataDir and metadata.port:
 * 1. Create isolated data directory with fresh database
 * 2. Start server instance on the specified port
 * 3. Wait for server to be ready
 *
 * Server PIDs are stored in .servers/ for teardown.
 */

import { FullConfig } from '@playwright/test';
import { execSync, spawn } from 'child_process';
import * as fs from 'fs';
import * as path from 'path';
import * as http from 'http';

const PROJECT_ROOT = path.resolve(__dirname, '../..');
const SERVERS_DIR = path.join(__dirname, '.servers');
const PY_SCRIPTS_DIR = path.join(PROJECT_ROOT, 'py_scripts');

interface ServerInfo {
  pid: number;
  port: number;
  dataDir: string;
}

// Kill any stale kr_notebook processes that might be left from previous runs
function killStaleServers(): void {
  console.log('  Cleaning up stale server processes...');

  // Find and kill any existing kr_notebook processes
  try {
    if (process.platform === 'win32') {
      execSync('taskkill /F /IM kr_notebook.exe 2>nul', { stdio: 'pipe' });
    } else {
      // Kill processes matching kr_notebook binary
      execSync('pkill -f "target.*kr_notebook" 2>/dev/null || true', { stdio: 'pipe' });
    }
  } catch {
    // Ignore errors - no processes to kill
  }

  // Also clean up any tracked PIDs from previous runs
  if (fs.existsSync(SERVERS_DIR)) {
    const files = fs.readdirSync(SERVERS_DIR);
    for (const file of files) {
      if (file.endsWith('.json')) {
        try {
          const infoPath = path.join(SERVERS_DIR, file);
          const info: ServerInfo = JSON.parse(fs.readFileSync(infoPath, 'utf-8'));
          if (info.pid) {
            try {
              process.kill(info.pid, 'SIGTERM');
              console.log(`    Killed stale server PID ${info.pid}`);
            } catch {
              // Process already dead
            }
          }
          fs.unlinkSync(infoPath);
        } catch {
          // Ignore corrupt files
        }
      }
    }
  }
}

// Ensure .servers directory exists
function ensureServersDir(): void {
  if (!fs.existsSync(SERVERS_DIR)) {
    fs.mkdirSync(SERVERS_DIR, { recursive: true });
  }
}

// Save server info for teardown
function saveServerInfo(projectName: string, info: ServerInfo): void {
  const infoPath = path.join(SERVERS_DIR, `${projectName}.json`);
  fs.writeFileSync(infoPath, JSON.stringify(info, null, 2));
}

// Initialize test environment using db-manager
function initTestEnv(name: string, dataDir: string): void {
  console.log(`  Initializing test environment: ${name}`);

  // Clean up if exists
  if (fs.existsSync(dataDir)) {
    fs.rmSync(dataDir, { recursive: true });
  }

  // Create environment using db-manager (which calls cargo run --init-db)
  execSync(
    `uv run db-manager init-test-env ${name} --data-dir "${dataDir}"`,
    {
      cwd: PY_SCRIPTS_DIR,
      stdio: 'pipe',
    }
  );
}

// Start server with isolated data directory
function startServer(projectName: string, dataDir: string, port: number): Promise<number> {
  return new Promise((resolve, reject) => {
    console.log(`  Starting server on port ${port} with DATA_DIR=${dataDir}`);

    const server = spawn('cargo', ['run', '--quiet'], {
      cwd: PROJECT_ROOT,
      env: {
        ...process.env,
        DATA_DIR: dataDir,
        PORT: port.toString(),
        RUST_LOG: 'warn', // Reduce log noise
      },
      detached: true,
      stdio: ['ignore', 'pipe', 'pipe'],
    });

    if (!server.pid) {
      reject(new Error('Failed to start server'));
      return;
    }

    // Detach from parent process
    server.unref();

    // Give it a moment to start
    setTimeout(() => {
      resolve(server.pid as number);
    }, 500);
  });
}

// Wait for server to be ready
async function waitForServer(url: string, timeoutMs: number = 60000): Promise<void> {
  const startTime = Date.now();

  while (Date.now() - startTime < timeoutMs) {
    try {
      await new Promise<void>((resolve, reject) => {
        const req = http.get(url, (res) => {
          // Any response means server is up (even redirects)
          resolve();
        });
        req.on('error', reject);
        req.setTimeout(1000, () => {
          req.destroy();
          reject(new Error('Timeout'));
        });
      });
      return;
    } catch {
      // Wait before retry
      await new Promise((r) => setTimeout(r, 500));
    }
  }

  throw new Error(`Server did not become ready at ${url} within ${timeoutMs}ms`);
}

async function globalSetup(config: FullConfig): Promise<void> {
  console.log('\n=== Playwright Global Setup ===\n');

  // Kill any stale servers from previous interrupted runs
  killStaleServers();

  ensureServersDir();

  // Track which ports/dataDirs we've set up (for shared projects)
  const setupPorts = new Set<number>();

  for (const project of config.projects) {
    const metadata = project.metadata as { dataDir?: string; port?: number } | undefined;

    if (!metadata?.dataDir || !metadata?.port) {
      // This project doesn't need its own server (e.g., browser-only variation)
      continue;
    }

    // Skip if we've already set up this port
    if (setupPorts.has(metadata.port)) {
      continue;
    }
    setupPorts.add(metadata.port);

    console.log(`Setting up project: ${project.name}`);

    const dataDir = path.isAbsolute(metadata.dataDir)
      ? metadata.dataDir
      : path.join(PROJECT_ROOT, metadata.dataDir);

    // 1. Initialize test environment
    initTestEnv(project.name, dataDir);

    // 2. Start server
    const pid = await startServer(project.name, dataDir, metadata.port);

    // 3. Save server info
    saveServerInfo(project.name, {
      pid,
      port: metadata.port,
      dataDir,
    });

    // 4. Wait for server to be ready
    const url = `http://localhost:${metadata.port}/login`;
    console.log(`  Waiting for server at ${url}...`);
    await waitForServer(url);
    console.log(`  Server ready!`);
  }

  console.log('\n=== Setup Complete ===\n');
}

export default globalSetup;
