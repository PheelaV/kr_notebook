# Profiling System

The application includes a compile-time conditional profiling system that produces JSONL output for tracing events and activity. When disabled (the default), profiling has zero runtime cost.

## Building with Profiling

```bash
# Normal build (no profiling, zero overhead)
cargo build --release

# Build with profiling enabled
cargo build --release --features profiling

# Run with profiling
cargo run --features profiling
```

## Output

When profiling is enabled, events are:

1. **Printed to console** with a `[PROFILE]` prefix for real-time monitoring
2. **Written to a JSONL file** in the `data/` directory with timestamped filenames:

```
data/profile_2025-12-25_103000.jsonl
```

Each server run creates a new log file.

### Console Output Example

```
[PROFILE] {"timestamp":"2025-12-25T10:30:00.123Z","event_type":{"type":"session_start","session_id":"20251225_103000"}}
[PROFILE] {"timestamp":"2025-12-25T10:30:01.456Z","event_type":{"type":"handler_start","route":"/study","method":"GET"}}
[PROFILE] {"timestamp":"2025-12-25T10:30:01.458Z","event_type":{"type":"db_query","operation":"select","table":"cards"}}
```

## Event Types

The profiler captures the following event types:

| Event | Description |
|-------|-------------|
| `session_start` | Server startup |
| `handler_start` | HTTP handler invoked (route, method) |
| `handler_end` | HTTP handler completed (route, status) |
| `db_query` | Database query started (operation, table) |
| `db_query_complete` | Database query finished (operation, rows returned) |
| `srs_calculation` | SRS algorithm invoked (algorithm, card_id, rating) |
| `card_selection` | Card selected for study (mode, excluded_sibling) |
| `answer_validation` | User answer validated (card_id, is_correct, hints_used) |
| `settings_update` | Settings changed (setting name, new value) |

## JSONL Format

Each line is a self-contained JSON object:

```json
{"timestamp":"2025-12-25T10:30:00.123456Z","event_type":{"type":"handler_start","route":"/study","method":"GET"},"duration_us":null,"metadata":null}
{"timestamp":"2025-12-25T10:30:00.125000Z","event_type":{"type":"db_query","operation":"select","table":"cards"},"duration_us":null,"metadata":null}
{"timestamp":"2025-12-25T10:30:00.128000Z","event_type":{"type":"db_query_complete","operation":"get_due_cards","rows":5},"duration_us":null,"metadata":null}
{"timestamp":"2025-12-25T10:30:00.130000Z","event_type":{"type":"srs_calculation","algorithm":"fsrs","card_id":42,"rating":4},"duration_us":null,"metadata":null}
```

## Inspecting Logs

### Using jq

[jq](https://jqlang.github.io/jq/) is the recommended tool for inspecting JSONL files.

**View all events (pretty-printed):**
```bash
cat data/profile_*.jsonl | jq .
```

**Count events by type:**
```bash
cat data/profile_*.jsonl | jq -s 'group_by(.event_type.type) | map({type: .[0].event_type.type, count: length})'
```

**List all handler invocations:**
```bash
cat data/profile_*.jsonl | jq 'select(.event_type.type == "handler_start")'
```

**Find all card selections:**
```bash
cat data/profile_*.jsonl | jq 'select(.event_type.type == "card_selection") | .event_type'
```

**View SRS calculations for a specific card:**
```bash
cat data/profile_*.jsonl | jq 'select(.event_type.type == "srs_calculation" and .event_type.card_id == 42)'
```

**Count database queries by operation:**
```bash
cat data/profile_*.jsonl | jq -s '[.[] | select(.event_type.type == "db_query")] | group_by(.event_type.operation) | map({op: .[0].event_type.operation, count: length})'
```

**Find incorrect answers:**
```bash
cat data/profile_*.jsonl | jq 'select(.event_type.type == "answer_validation" and .event_type.is_correct == false)'
```

**Get timeline of events (timestamps only):**
```bash
cat data/profile_*.jsonl | jq -r '[.timestamp, .event_type.type] | @tsv'
```

### Using grep

For quick searches without jq:

```bash
# Find all handler events
grep '"handler_start"' data/profile_*.jsonl

# Find FSRS calculations
grep '"srs_calculation"' data/profile_*.jsonl

# Find failed validations
grep '"is_correct":false' data/profile_*.jsonl
```

### Using Python

```python
import json

with open('data/profile_2025-12-25_103000.jsonl') as f:
    events = [json.loads(line) for line in f]

# Count events by type
from collections import Counter
event_types = Counter(e['event_type']['type'] for e in events)
print(event_types)

# Find slow database queries (if duration tracking is enabled)
slow_queries = [e for e in events
                if e['event_type']['type'] == 'db_query_complete'
                and e.get('duration_us', 0) > 10000]
```

## Architecture

The profiling system uses compile-time feature flags:

```
┌─────────────────────────────────────────────────┐
│              Application Code                    │
│  Handlers → DB → SRS → Validation               │
│         │                                        │
│         ▼                                        │
│   profile_log!(EventType::...)                  │
│         │                                        │
│    ┌────┴────┐                                  │
│    │         │                                  │
│ profiling  not(profiling)                       │
│    │         │                                  │
│    ▼         ▼                                  │
│ logger.rs  noop.rs                              │
│    │       (empty)                              │
│    ▼                                            │
│ profile_*.jsonl                                 │
└─────────────────────────────────────────────────┘
```

When `--features profiling` is not specified, all `profile_log!` macro calls compile to nothing.
