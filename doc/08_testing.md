# Testing Guide

This document describes the testing infrastructure for kr_notebook.

## Quick Reference

| Type | Command | Location | Framework |
|------|---------|----------|-----------|
| Unit | `cargo test` | `src/` (inline) | Rust built-in |
| Integration | `cd tests/integration && ./run_tests.sh` | `tests/integration/` | pytest + httpx |
| E2E | `cd tests/e2e && npm test` | `tests/e2e/` | Playwright |

## Architecture

```
┌─────────────────────────────────────────────────────┐
│ E2E (Playwright)                                    │
│ Browser automation, user flows, UI interactions     │
│ 9 test suites on ports 3001-3009                    │
├─────────────────────────────────────────────────────┤
│ Integration (pytest + httpx)                        │
│ HTTP API testing, session handling, database state  │
│ Single server on port 3100                          │
├─────────────────────────────────────────────────────┤
│ Unit (cargo test)                                   │
│ Pure functions, algorithms, data structures         │
│ No server required                                  │
└─────────────────────────────────────────────────────┘
```

## Unit Tests

**207 tests** across 22 modules with `#[cfg(test)]` blocks.

### Running

```bash
cargo test                    # All tests
cargo test test_name          # Filter by name
cargo test -- --nocapture     # Show println! output
cargo test --features profiling  # With profiling enabled
```

### Key Test Modules

| Module | Coverage |
|--------|----------|
| `src/srs/fsrs_scheduler.rs` | FSRS algorithm, learning steps, graduation |
| `src/srs/card_selector.rs` | Weight calculation, reinforcement queue |
| `src/validation.rs` | Answer matching, typo tolerance, variants |
| `src/auth/password.rs` | Argon2 hashing, verification |
| `src/content/discovery.rs` | Pack discovery, manifest parsing |

### Adding Unit Tests

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_example() {
        let result = your_function(input);
        assert_eq!(result, expected);
    }
}
```

---

## Integration Tests

**7 test modules** testing the full HTTP API with isolated database.

### Setup

```bash
cd tests/integration
uv sync                       # Install dependencies (first time)
./run_tests.sh                # Run all tests (manages server)
```

### Manual Running

```bash
uv run pytest                 # Requires server already running
uv run pytest -k auth         # Filter by name
uv run pytest -v              # Verbose output
uv run pytest --tb=short      # Shorter tracebacks
```

### Test Isolation

```
┌──────────────────────────────────────────────────┐
│ test_server fixture (session scope)              │
│                                                   │
│  1. Creates data/test/integration/               │
│  2. Initializes via db-manager init-test-env     │
│  3. Spawns cargo run on port 3100                │
│  4. Tests run against isolated server            │
│  5. Cleanup (unless PRESERVE_TEST_ENV=1)         │
└──────────────────────────────────────────────────┘
```

### Key Fixtures

| Fixture | Scope | Purpose |
|---------|-------|---------|
| `test_server` | session | Spawns isolated server |
| `db_manager` | session | CLI wrapper for user/scenario management |
| `client` | function | HTTP client (no session) |
| `authenticated_client` | function | HTTP client with logged-in session |
| `test_user` | function | Creates user, yields `(username, password_hash)`, cleans up |
| `admin_user` | function | Creates admin user |

### Test Files

| File | Coverage |
|------|----------|
| `test_auth.py` | Login, logout, protected routes, sessions |
| `test_study.py` | Study flow, card review, SRS integration |
| `test_admin_security.py` | Admin access control, role enforcement |
| `test_tiers.py` | Tier progression, unlock mechanics |
| `test_packs.py` | Content pack enable/disable |
| `test_data_isolation.py` | Per-user database isolation |
| `test_groups.py` | Group CRUD, membership |

### Adding Integration Tests

```python
# tests/integration/tests/test_example.py

def test_protected_route_requires_auth(client):
    """Unauthenticated requests redirect to login."""
    response = client.get("/settings")
    assert response.status_code == 302
    assert "/login" in response.headers["location"]

def test_authenticated_access(authenticated_client):
    """Authenticated users can access protected routes."""
    response = authenticated_client.get("/settings")
    assert response.status_code == 200
```

---

## E2E Tests

**11 test suites** with full browser automation across Chrome, Firefox, and WebKit.

### Setup

```bash
cd tests/e2e
npm install                   # Install dependencies (first time)
npx playwright install        # Install browsers (first time)
```

### Running

```bash
# All suites, all browsers (default for CI)
npm test

# Single browser (recommended for dev)
BROWSER=chrome npm test
BROWSER=firefox npm test
BROWSER=webkit npm test

# Single suite + browser
npx playwright test --project=auth-chrome
npx playwright test --project=fresh-install-firefox

# Other modes
npm run test:headed           # With visible browser
npm run test:ui               # Interactive Playwright UI
npm run test:debug            # Step-through debugging
```

### Test Isolation

Each suite+browser combination runs in complete isolation with its own server and database:

```
┌──────────────────────────────────────────────────────────────────┐
│ Project              │ Port  │ Data Directory                    │
├──────────────────────────────────────────────────────────────────┤
│ auth-chrome          │ 3001  │ data/test/e2e-auth-chrome         │
│ auth-firefox         │ 3002  │ data/test/e2e-auth-firefox        │
│ auth-webkit          │ 3003  │ data/test/e2e-auth-webkit         │
│ study-chrome         │ 3004  │ data/test/e2e-study-chrome        │
│ study-firefox        │ 3005  │ data/test/e2e-study-firefox       │
│ ...                  │ ...   │ ...                               │
│ fresh-install-chrome │ 3031  │ data/test/e2e-fresh-install-chrome│
│ fresh-install-firefox│ 3032  │ data/test/e2e-fresh-install-firefox│
│ fresh-install-webkit │ 3033  │ data/test/e2e-fresh-install-webkit│
└──────────────────────────────────────────────────────────────────┘

Port formula: BASE_PORT (3001) + suite_index * 3 + browser_index
- browser_index: chrome=0, firefox=1, webkit=2
```

This ensures browsers never interfere with each other's test state.

### Test Suites

| Suite | Spec File | Coverage |
|-------|-----------|----------|
| auth | `auth.spec.ts` | Login form, credentials, logout, redirects |
| study | `study.spec.ts` | Interactive study, classic mode, hints |
| registration | `registration.spec.ts` | User registration flow |
| admin | `admin.spec.ts` | Admin access control, role management |
| groups | `groups.spec.ts` | Group creation, membership, permissions |
| pack-permissions | `pack-permissions.spec.ts` | Pack visibility controls |
| settings | `settings.spec.ts` | User settings, data export/import |
| menu-visibility | `menu-visibility.spec.ts` | Conditional UI based on roles |
| navbar-dropdown | `navbar-dropdown.spec.ts` | Navbar dropdown consistency |
| offline-study | `offline-study.spec.ts` | Offline study mode (download, study, sync) |
| fresh-install | `fresh-install.spec.ts` | Fresh installation, default admin creation |

### Adding E2E Tests

```typescript
// tests/e2e/specs/example.spec.ts
import { test, expect } from '@playwright/test';

test('user can log in', async ({ page }) => {
  await page.goto('/login');
  await page.fill('input[name="username"]', 'testuser');
  await page.fill('input[name="password"]', 'password123');
  await page.click('button[type="submit"]');
  await expect(page).toHaveURL('/');
});
```

### Fixtures

Located in `tests/e2e/fixtures/`:
- `auth.ts` - User creation, login helpers, role assignment
- Server management via `global-setup.ts` and `global-teardown.ts`

---

## Troubleshooting

### Port Conflicts

```bash
# Find process on port
lsof -i :3001

# Kill stale server
kill -9 <PID>

# E2E cleanup (auto-kills stale servers)
cd tests/e2e && rm -rf .servers/
```

### Integration Server Not Starting

```bash
# Check if server compiles
cargo build

# Check data directory
ls -la data/test/integration/

# Preserve env for debugging
PRESERVE_TEST_ENV=1 ./run_tests.sh
```

### E2E Timeouts

```bash
# Increase timeout in playwright.config.ts
timeout: 60000,

# Debug specific test
npx playwright test --debug auth.spec.ts
```

### Flaky Tests

```bash
# Run with retries
npx playwright test --retries=3

# Generate trace on failure (in config)
trace: 'on-first-retry',
```

---

## CI/CD

Tests can be run in CI with:

```bash
# Unit tests
cargo test

# Integration (requires cargo build first)
cd tests/integration && ./run_tests.sh

# E2E (all browsers - default)
cd tests/e2e && npm ci && npx playwright install --with-deps && npm test

# E2E (single browser - faster for quick checks)
cd tests/e2e && npm ci && npx playwright install chromium && BROWSER=chrome npm test
```

---

## Source Files

| File | Purpose |
|------|---------|
| `tests/e2e/playwright.config.ts` | E2E configuration, project definitions |
| `tests/e2e/global-setup.ts` | Server startup for E2E |
| `tests/e2e/global-teardown.ts` | Server cleanup for E2E |
| `tests/integration/conftest.py` | pytest fixtures, server management |
| `tests/integration/run_tests.sh` | Integration test runner script |
