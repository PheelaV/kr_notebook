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
- **Mobile-Responsive**: Hamburger menu, touch-friendly buttons, adaptive layouts
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

## Usage

| Route | Description |
|-------|-------------|
| `/` | Home - cards due, quick stats |
| `/study` | Interactive study session |
| `/progress` | Detailed progress by tier, problem areas |
| `/settings` | Algorithm settings, tier selection, audio management |
| `/library` | Browse all unlocked characters |
| `/reference` | Hangul reference charts |
| `/pronunciation` | Interactive syllable audio matrix (if audio available) |
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
├── Dockerfile              # Multi-stage Rust build
├── docker-compose.yml      # LAN deployment
├── src/                    # Rust backend
│   ├── main.rs             # Server entry point
│   ├── lib.rs              # Module exports
│   ├── paths.rs            # Centralized path constants
│   ├── db/                 # Database layer
│   │   ├── mod.rs          # Card seed data
│   │   ├── schema.rs       # Table definitions
│   │   └── repository.rs   # CRUD operations
│   ├── domain/             # Data models
│   │   └── card.rs         # Card struct
│   ├── handlers/           # HTTP handlers
│   │   ├── mod.rs          # Index handler
│   │   ├── study.rs        # Study session logic
│   │   ├── progress.rs     # Progress & analytics
│   │   ├── settings.rs     # Settings + audio management
│   │   ├── pronunciation.rs # Pronunciation matrix
│   │   └── library.rs      # Character library
│   ├── profiling/          # Optional profiling (--features profiling)
│   ├── srs/                # Spaced repetition (FSRS + SM-2)
│   └── validation.rs       # Answer validation
├── py_scripts/             # Python scraper tools
│   ├── pyproject.toml      # uv/pip project config
│   ├── uv.lock             # Locked dependencies
│   └── src/kr_scraper/     # Scraper package
│       ├── cli.py          # CLI commands (lesson1, lesson2, segment)
│       ├── paths.py        # Centralized path constants
│       ├── segment.py      # Audio segmentation logic
│       ├── lesson1.py      # Lesson 1 scraper
│       ├── lesson2.py      # Lesson 2 scraper
│       └── utils.py        # HTTP/parsing utilities
├── templates/              # Askama HTML templates
├── doc/                    # Documentation
└── data/                   # Runtime data (gitignored, except manifests)
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
