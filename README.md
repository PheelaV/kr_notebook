# Korean Hangul Learning App

A Rust web application for learning Korean Hangul using spaced repetition with the modern FSRS algorithm and interactive answer validation.

## Features

- **FSRS Algorithm**: Modern Free Spaced Repetition Scheduler (20-30% more efficient than SM-2)
- **Interactive Learning**: Type romanization or select Korean from multiple choice - no passive reveal-and-rate
- **Progressive Hints**: 3-level hint system (length → description → partial reveal)
- **Confusion Tracking**: Identifies problem characters and common mistakes
- **Tiered Learning**: Progress from basic to advanced characters
  - Tier 1: Basic consonants (ㄱ, ㄴ, ㄷ...) and vowels (ㅏ, ㅓ, ㅗ...)
  - Tier 2: Y-vowels (ㅑ, ㅕ...) and special ieung (ㅇ)
  - Tier 3: Aspirated (ㅋ, ㅍ...) and tense consonants (ㄲ, ㅃ...)
  - Tier 4: Compound vowels (ㅘ, ㅝ...)
- **Accelerated Mode**: Unlock all tiers immediately for experienced learners
- **Focus Mode**: Study specific tiers only
- **Listening Practice**: Audio recognition with syllable playback
- **Practice Mode**: Untracked learning without affecting SRS
- **Mobile-Responsive**: Hamburger menu, touch-friendly buttons, double-tap submit
- **Haetae Mascot**: Animated Korean guardian companion

## Tech Stack

- **Backend**: Axum (async web framework)
- **Database**: SQLite via rusqlite
- **Templating**: Askama (compile-time templates)
- **Frontend**: HTMX 2.x + Tailwind CSS (CDN)
- **SRS**: FSRS 5.2 (with SM-2 fallback)

## Getting Started

### Option 1: Docker (Recommended)

The easiest way to run the app, especially for LAN hosting:

```bash
git clone https://github.com/PheelaV/kr_notebook.git
cd kr_notebook
docker compose up -d
```

Access at `http://localhost:3000` or `http://<your-ip>:3000` on your LAN.

```bash
# View logs
docker compose logs -f

# Stop
docker compose down
```

The database persists in `./data/hangul.db`.

### Option 2: Native Rust

#### Prerequisites

- Rust 1.88+

#### Installation

```bash
git clone https://github.com/PheelaV/kr_notebook.git
cd kr_notebook
cargo run
```

The server starts at `http://localhost:3000`

### Development

```bash
# Hot reload
cargo install cargo-watch
cargo watch -x run

# Tests
cargo test

# Lint
cargo clippy
```

## Configuration

Database path is configurable (priority order):
1. `config.toml` → `[database] path = "..."`
2. `DATABASE_PATH` environment variable
3. Default: `data/hangul.db`

```bash
cp config.toml.example config.toml  # Optional local config
```

## Usage

| Route | Description |
|-------|-------------|
| `/` | Home - cards due, stats, countdown |
| `/study` | Interactive study (type/select) |
| `/study-classic` | Classic reveal-and-rate mode |
| `/practice` | Untracked practice |
| `/listen` | Listening practice (audio) |
| `/progress` | Progress by tier, problem areas |
| `/settings` | Algorithm, tiers, audio config |
| `/library` | Browse unlocked characters |
| `/reference` | Hangul reference charts |
| `/pronunciation` | Syllable audio matrix |
| `/guide` | How to use the app |

### Study Flow

1. See a Korean character (e.g., ㄱ)
2. Type the romanization (e.g., "g" or "k") or select from choices
3. Use hints if stuck (counts as "Hard")
4. System auto-rates based on correctness
5. FSRS schedules next review optimally

## Project Structure

```
kr_notebook/
├── Cargo.toml
├── build.rs                # Compile-time asset hashing
├── askama.toml             # Askama configuration
├── Dockerfile              # Multi-stage Rust build
├── docker-compose.yml      # LAN deployment
├── src/                    # Rust backend
│   ├── main.rs             # Server entry point
│   ├── lib.rs              # Module exports
│   ├── paths.rs            # Centralized path constants
│   ├── config.rs           # Configuration loading
│   ├── audio.rs            # Audio file handling
│   ├── session.rs          # Session management
│   ├── filters.rs          # Template filters
│   ├── validation.rs       # Answer validation
│   ├── db/                 # Database layer
│   │   ├── mod.rs          # Pool management, seed data
│   │   ├── schema.rs       # Table definitions
│   │   ├── cards.rs        # Card queries
│   │   ├── reviews.rs      # Review operations
│   │   ├── stats.rs        # Statistics
│   │   └── tiers.rs        # Tier progress
│   ├── domain/             # Data models
│   ├── handlers/           # HTTP handlers
│   │   ├── mod.rs          # Index, exports
│   │   ├── study.rs        # Study session
│   │   ├── listen.rs       # Listening practice
│   │   ├── progress.rs     # Progress analytics
│   │   ├── settings.rs     # Settings + audio
│   │   ├── pronunciation.rs # Audio matrix
│   │   ├── library.rs      # Character browser
│   │   ├── reference.rs    # Reference pages
│   │   └── guide.rs        # Usage guide
│   ├── profiling/          # Optional (--features profiling)
│   └── srs/                # Spaced repetition (FSRS + SM-2)
├── py_scripts/             # Python tools
│   ├── pyproject.toml
│   └── src/
│       ├── kr_scraper/     # Audio scraper
│       └── db_manager/     # Database scenarios CLI
├── templates/              # Askama HTML templates
├── doc/                    # Documentation
└── data/                   # Runtime data (gitignored)
    └── scraped/htsk/       # Scraped audio + manifests
```

## Algorithms

### FSRS (Primary)

Free Spaced Repetition Scheduler - tracks memory stability and difficulty per card:
- **Rating**: Again (1), Hard (2), Good (3), Easy (4)
- **Retention target**: 90% (configurable)
- More details: [open-spaced-repetition/fsrs-rs](https://github.com/open-spaced-repetition/fsrs-rs)

### SM-2 (Fallback)

Classic SuperMemo 2 algorithm:
- **Rating**: Again (0), Hard (2), Good (4), Easy (5)
- **Ease factor**: Adjusts based on performance (min 1.3)

## Pronunciation Audio

The app supports pronunciation audio from howtostudykorean.com (Lessons 1-2).

### Setup (requires Python 3.12+ and uv)

```bash
cd py_scripts
uv sync                      # Install dependencies
uv run kr-scraper lesson1    # Download Lesson 1 audio
uv run kr-scraper lesson2    # Download Lesson 2 audio
uv run kr-scraper segment    # Segment into syllables
```

### Docker Note

The Docker image is Rust-only and does not include Python/uv. To use pronunciation
audio with Docker, run the scraper on your host machine first:

```bash
# On host (before docker compose up)
cd py_scripts && uv sync && uv run kr-scraper lesson1 && uv run kr-scraper lesson2 && uv run kr-scraper segment
cd .. && docker compose up -d
```

The `data/` directory is mounted into the container, so scraped audio will be available.

### Manifest Distribution

Manifests (`data/scraped/htsk/*/manifest.json`) contain segmentation parameters
and are version-controlled. After cloning, regenerate audio using the saved parameters:

```bash
cd py_scripts
uv run kr-scraper lesson1 && uv run kr-scraper lesson2 && uv run kr-scraper segment
```

### Per-Row Tuning

Settings → Pronunciation Audio → Preview allows adjusting parameters per row:
- **s**: Min silence (ms) - gap detection threshold
- **t**: Threshold (dBFS) - silence detection sensitivity
- **P**: Padding (ms) - buffer before/after segments
- **skip**: Skip first N segments (for noisy audio)

### Database Management

Manage database scenarios for testing:

```bash
uv run db-manager status      # Show current database
uv run db-manager list        # List scenarios
uv run db-manager use <name>  # Switch scenario
uv run db-manager create <name>  # Create from current
uv run db-manager reset       # Reset to golden
```

## Profiling

Enable profiling to log handler timing and DB queries:

```bash
cargo run --features profiling
```

Outputs:
- Console: `[PROFILE] {...}` JSON lines
- File: `data/profile_{timestamp}.jsonl`

## Documentation

- [`doc/01_learning_fsa.md`](doc/01_learning_fsa.md) - Learning mode state machine (normal vs accelerated)
- [`doc/02_responsiveness_guidance.md`](doc/02_responsiveness_guidance.md) - Mobile responsiveness patterns

## Attribution

Pronunciation audio is sourced from [How to Study Korean](https://www.howtostudykorean.com/), an excellent free resource for learning Korean:

- [Unit 0 Lesson 1](https://www.howtostudykorean.com/unit0/unit0lesson1/) - Basic consonants (ㅂㅈㄷㄱㅅㅁㄴㅎㄹ) and vowels (ㅣㅏㅓㅡㅜㅗ)
- [Unit 0 Lesson 2](https://www.howtostudykorean.com/unit0/unit-0-lesson-2/) - Tense (ㄲㅃㅉㄸㅆ) and aspirated (ㅋㅍㅊㅌ) consonants

Audio files are not redistributed with this project. Users must run the scraper to download audio for personal educational use.

## License

MIT
