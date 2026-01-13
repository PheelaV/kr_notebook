# Offline Study Mode

Offline mode enables studying without an internet connection by running the FSRS algorithm client-side via WebAssembly.

## Overview

- **Status**: Experimental (disabled by default)
- **Toggle**: Settings → Enable offline study
- **Compatibility**: Requires WASM, IndexedDB, and Service Worker support

## How It Works

1. **Prepare Session**: Download cards for offline study (Settings page)
2. **Study Offline**: WASM module runs validation + FSRS scheduling locally
3. **Sync Back**: When online, sync reviews to server

## Architecture

### WASM Module (`crates/offline-srs/`)

Contains browser-compatible versions of:
- Answer validation (`validate_answer`)
- FSRS scheduling (`calculate_next_review`)
- Hint generation (`get_hint`)

Built with `wasm-pack` using `rs-fsrs` (scheduler-only, no Burn dependency).

```
static/wasm/
├── offline_srs_bg.wasm  (~200KB, ~70KB gzip)
└── offline_srs.js       (~10KB)
```

### Build

```bash
./scripts/build-wasm.sh
```

Prerequisites: `cargo install wasm-pack`

### API

```javascript
import init, { validate_answer, calculate_next_review, get_hint } from '/static/wasm/offline_srs.js';

// Initialize WASM module
await init();

// Validate an answer
const result = validate_answer("g", "g / k", false);
// Returns JSON: {"result": "Correct", "quality": 4, "is_correct": true}

// Calculate next review
const cardState = JSON.stringify({
  learning_step: 0,
  stability: null,
  difficulty: null,
  repetitions: 0,
  next_review: new Date().toISOString()
});
const scheduling = calculate_next_review(cardState, 4, 0.9, false);
// Returns JSON with new SRS state

// Get progressive hints
const hint1 = get_hint("안녕하세요", 1); // "안____ (5 letters)"
const hint2 = get_hint("안녕하세요", 2); // "안녕___"
```

## Data Flow

### Download Session

```
POST /api/study/download-session
{
  "duration_minutes": 30,
  "filter_mode": "all"
}
→ Session with cards + current SRS state
```

### Offline Study Loop

1. Display card from IndexedDB
2. User answers → `validate_answer()` (WASM)
3. `calculate_next_review()` → new SRS state (WASM)
4. Store response in IndexedDB
5. Failed cards → reinforcement queue
6. Select next card

### Sync

When coming back online, an auto-sync modal appears prompting to sync pending reviews.

```
POST /api/study/sync-offline
{
  "session_id": "...",
  "reviews": [
    {
      "card_id": 42,
      "quality": 4,
      "is_correct": true,
      "hints_used": 0,
      "timestamp": "2024-01-13T10:05:00Z",
      ...
    }
  ]
}
```

**Important**: The server recalculates `next_review` using its own FSRS algorithm based on the offline review `timestamp`. Client-provided SRS values are not trusted. This ensures accurate scheduling even if sync happens hours after the offline session.

## Files

| File | Purpose |
|------|---------|
| `crates/offline-srs/` | WASM crate source |
| `scripts/build-wasm.sh` | Build script |
| `static/wasm/` | Built WASM + JS |
| `static/js/offline-study.js` | Client-side study controller |
| `static/js/offline-storage.js` | IndexedDB storage for sessions/responses |
| `static/js/offline-sync.js` | Auto-sync logic and UI |
| `static/js/offline-detect.js` | Feature detection |
| `src/handlers/study/offline.rs` | Server endpoints (download, sync) |

## Testing

```bash
# Unit tests for WASM crate
cd crates/offline-srs && cargo test

# Build WASM
./scripts/build-wasm.sh

# E2E: DevTools → Network → Offline → Study
```

## Limitations

- No audio playback offline (future enhancement)
- Tier/lesson unlocks deferred until sync
- Session expires after 48 hours (stale warning)
- One active offline session per user

## Why rs-fsrs vs fsrs?

| Crate | Dependencies | WASM Size | Use Case |
|-------|--------------|-----------|----------|
| `fsrs` | Burn (ML framework) | ~5MB+ | Training optimizer |
| `rs-fsrs` | chrono only | ~200KB | Scheduling only |

For offline study, we only need scheduling (not training), so `rs-fsrs` is ideal.
