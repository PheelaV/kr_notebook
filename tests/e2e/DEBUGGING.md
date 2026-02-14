# E2E Test Debugging Guide

Practical guide for diagnosing and fixing flaky Playwright tests in this project. Based on real debugging sessions that took offline Firefox tests from 10+ failures to 0.

## Quick Commands

```bash
cd tests/e2e

# Run a single test (fastest feedback loop)
BROWSER=firefox npx playwright test --project=offline-study-firefox -g "test name" --workers=1

# Repeat a single test to check for flakiness
BROWSER=firefox npx playwright test --project=offline-study-firefox -g "test name" --repeat-each=5 --workers=1

# Run one suite on one browser
BROWSER=firefox npx playwright test --project=offline-study-firefox --workers=1

# Run all Firefox tests
BROWSER=firefox npx playwright test

# Open HTML report after failures
npx playwright show-report
```

## Debugging Workflow

**Rule: tight feedback loops.** Run the smallest failing unit first, fix it, then widen scope.

1. **Single test, single browser** — isolate the failure
2. **Repeat with `--repeat-each=5`** — confirm it's flaky vs. deterministic
3. **Single test file** — check if tests within a file interfere
4. **Multiple files** — check cross-file interactions
5. **Full suite** — final validation

## Known Gotchas

### 1. Firefox BFCache Serves Stale Pages

**Symptom:** A form POST succeeds (server persists the data), but the subsequent page load shows old data. Especially after `page.goto()` to the same URL.

**Root cause:** Firefox's back-forward cache (BFCache) serves a cached version of the page instead of making a fresh request to the server.

**Fix:** Add a cache-busting query parameter:

```typescript
// BAD — Firefox may serve cached page
await page.goto('/settings');

// GOOD — forces fresh server render
await page.goto('/settings?_t=' + Date.now());
```

**When to suspect this:** The server logs show the setting was saved, intercepted responses confirm success, but the DOM shows old values. Passes on Chrome, fails on Firefox.

### 2. Form POST via fetch() Instead of Click

**Symptom:** Checkbox + form submit is unreliable in Firefox. The POST data is correct but the setting doesn't persist intermittently.

**Root cause:** Firefox's form submission + 302 redirect + page reload chain has timing edge cases where `page.goto()` after form submit can serve cached content.

**Fix:** Use `page.evaluate(fetch())` for the POST, then navigate with cache-busting:

```typescript
// RELIABLE approach: fetch POST + cache-busted navigation
await page.evaluate(async () => {
  const body = new URLSearchParams();
  body.append('_action', 'offline_mode');
  body.append('offline_mode_enabled', 'true');
  await fetch('/settings', {
    method: 'POST',
    headers: { 'Content-Type': 'application/x-www-form-urlencoded' },
    body: body.toString(),
    redirect: 'follow',
  });
});
await page.goto('/settings?_t=' + Date.now(), { waitUntil: 'domcontentloaded' });
```

This is used in `enableOfflineMode()` helpers in both `offline-study.spec.ts` and `offline-sync.spec.ts`.

### 3. MCQ Requires Two-Step Submission

**Symptom:** Offline study tests fail at "expect result section to be visible" — the card shows MCQ options but the answer was never submitted.

**Root cause:** The MCQ UI uses a select-then-confirm pattern:
1. Click `.choice-btn` to select an answer
2. Click `.mcq-submit-btn` ("Check") to submit

Tests that only do step 1 leave the card in "selected but not submitted" state.

**Fix:**

```typescript
// BAD — selects but doesn't submit
await choiceBtn.click();

// GOOD — selects AND submits
await choiceBtn.click();
await page.locator('.mcq-submit-btn').click();
```

Note: `dblclick()` on a choice button auto-submits (the JS has double-tap detection), which is why some tests using `dblclick()` worked fine.

### 4. HTMX Swap Races with `isVisible()` Checks

**Symptom:** A test that answers multiple study cards in a loop fails intermittently. The error context shows an MCQ card with the "Check" button still disabled, and `result-phase` never appears.

**Root cause:** After HTMX swaps in a new card, instantaneous `isVisible()` checks can execute before the new DOM is fully settled. Both the text-input and choice-grid branches are skipped, so no answer is submitted. Additionally, clicking a choice button before `card-interactions.js` event delegation fires means `selectAnswer()` never runs and the submit button stays disabled.

**Fix:** Use retrying assertions (`expect().toBeVisible()`) instead of instant `isVisible()` to wait for the new card's input elements. After clicking an MCQ choice, wait for the submit button to become enabled (proves the JS handler ran):

```typescript
async function answerCurrentCard(page) {
  const textInput = page.locator('[data-testid="answer-input"]');
  const choiceGrid = page.locator('[data-testid="choice-grid"]');

  // Retrying assertion — survives HTMX swap timing
  await expect(textInput.or(choiceGrid)).toBeVisible({ timeout: 10000 });

  if (await choiceGrid.isVisible()) {
    await page.locator('[data-testid="choice-option"]').first().click();
    // Wait for submit button to be enabled (proves JS event handler ran)
    const submitBtn = page.locator('[data-testid="submit-answer"]');
    await expect(submitBtn).toBeEnabled({ timeout: 5000 });
    await submitBtn.click();
  } else {
    await textInput.fill('test');
    await page.locator('[data-testid="submit-answer"]').click();
  }

  await expect(page.locator('[data-testid="result-phase"]')).toBeVisible({ timeout: 10000 });
}
```

**General rule:** Never use bare `isVisible()` to choose a code path after a navigation or HTMX swap. Always gate on a retrying `expect().toBeVisible()` first to ensure the DOM has settled.

### 6. SQLite Contention with External Tools

**Symptom:** Tests that call `setupScenario()` (which invokes Python `db-manager`) before browser interactions fail more than tests without it.

**Root cause:** The Python `db-manager` CLI opens the user's `learning.db`, writes data, and closes it. If `conn.commit()` is missing before `conn.close()`, Python's default isolation level means writes may not be flushed. The Rust server then reads stale data.

**Fix:** Ensure all Python CLI commands that write to the database call `conn.commit()` before `conn.close()`. Check `py_scripts/src/db_manager/cli.py` — every `conn.close()` in a `finally` block should have a corresponding `conn.commit()` if the function wrote data.

### 7. `fullyParallel: false` for SQLite-Heavy Suites

**Symptom:** Tests pass individually but fail when run together. Error messages about database being locked or settings not persisting.

**Root cause:** Multiple tests writing to the same SQLite database simultaneously. SQLite has limited concurrent write support, especially without WAL mode or `busy_timeout`.

**Fix:** Set `fullyParallel: false` in `playwright.config.ts` for affected suites:

```typescript
{ name: 'offline-study', testMatch: 'offline-study.spec.ts', fullyParallel: false },
{ name: 'offline-sync', testMatch: 'offline-sync.spec.ts', fullyParallel: false },
```

## Diagnostic Techniques

### Capture Fetch Response HTML

When a form POST seems to fail, verify server-side by checking the response HTML directly:

```typescript
const result = await page.evaluate(async () => {
  const resp = await fetch('/settings', {
    method: 'POST',
    headers: { 'Content-Type': 'application/x-www-form-urlencoded' },
    body: '_action=offline_mode&offline_mode_enabled=true',
    redirect: 'follow',
  });
  const html = await resp.text();
  return {
    status: resp.status,
    // Check specific element state in response HTML
    hasChecked: html.includes('offlineModeToggle') && html.includes('checked'),
  };
});
```

If `hasChecked` is true but the page shows unchecked — it's a caching issue (see gotcha #1).

### Write Diagnostics to Files

`console.log` in tests doesn't always appear in CI output. Write to `/tmp` instead:

```typescript
const fs = require('fs');
fs.writeFileSync(`/tmp/diag_${Date.now()}.json`, JSON.stringify(debugInfo, null, 2));
```

### Check Error Context Snapshots

After failures, Playwright saves page snapshots in `test-results/`:

```bash
# Find error context files from recent failures
find test-results -name "error-context.md" | head -5

# Look for key elements in the snapshot
grep -n "checkbox\|hidden\|button\|error" test-results/*/error-context.md
```

These show the exact DOM state at the moment of failure — invaluable for understanding what the page looked like.

### Use `--repeat-each` to Reproduce Flaky Tests

```bash
# Run 5 times to catch intermittent failures
npx playwright test -g "test name" --repeat-each=5 --workers=1
```

If it passes 5/5 for a single test but fails in the full suite, the issue is cross-test interference, not the test itself.

### Isolate Browser vs. Server Issues

If a test fails on Firefox but passes on Chrome:
1. Check BFCache (gotcha #1)
2. Check `force: true` click behavior on checkboxes inside labels
3. Check `waitForTimeout` values (Firefox is slower)

If a test fails on all browsers:
1. The issue is likely server-side or in the test logic
2. Check SQLite contention, missing commits, or incorrect selectors

## Architecture Notes

### Test Isolation

Each test suite + browser combination gets:
- **Unique port** (e.g., offline-study-firefox = port 3029)
- **Unique data directory** (e.g., `data/test/e2e-offline-study-firefox/`)
- **Separate Rust server instance** (started in `global-setup.ts`)

Within a suite, each test gets:
- **Unique user** via `createTestUser()` (username includes timestamp + random suffix)
- **Fresh browser context** via `authenticatedPage` fixture

### Server Logging

Servers start with `RUST_LOG=warn`. To get more detail, temporarily change `global-setup.ts`:

```typescript
env: {
  RUST_LOG: 'info',  // or 'debug' for maximum detail
  // ...
}
```

The Rust handler uses `.log_warn()` which logs at warn level when database writes fail — these would appear even with the default `RUST_LOG=warn`.

### Helper Functions

Two key helpers are shared across offline test files:

- **`enableOfflineMode(page)`** — POSTs offline_mode_enabled=true via fetch, navigates with cache-busting
- **`downloadSession(page)`** — Calls enableOfflineMode, then clicks download and waits for completion

The inline test `can enable offline mode in settings` uses the original form-click approach (for testing the actual UI flow). The helpers use the fetch approach (for reliability as a setup step).
