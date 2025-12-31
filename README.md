# Korean Hangul Learning App

A self-hosted Rust web application for learning Korean Hangul using spaced repetition with the modern FSRS algorithm. Multi-user support with per-user databases.

## Features

- **Multi-User Support**: User registration/login with isolated per-user databases
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
- **Self-Hosted**: No cloud dependencies, runs on your own hardware
- **Mobile-Responsive**: Hamburger menu, touch-friendly buttons, double-tap submit
- **Haetae Mascot**: Animated Korean guardian companion

## Tech Stack

- **Backend**: Axum (async web framework)
- **Database**: SQLite via rusqlite
- **Templating**: Askama (compile-time templates)
- **Frontend**: HTMX 2.x + Tailwind CSS (build-time)
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

User data persists in `./data/`.

### Self-Hosting with Tailscale

Run the app on a home server and access it securely from anywhere using [Tailscale](https://tailscale.com/):

1. **Install Tailscale** on your server and devices:
   ```bash
   # On your server (Linux)
   curl -fsSL https://tailscale.com/install.sh | sh
   sudo tailscale up
   ```

2. **Run the app** (Docker or native):
   ```bash
   docker compose up -d
   ```

3. **Access from any device** on your Tailnet:
   ```
   http://<server-tailscale-ip>:3000
   ```
   Find your server's Tailscale IP with `tailscale ip` or in the Tailscale admin console.

4. **Optional: Use MagicDNS** for a friendly hostname:
   ```
   http://<server-hostname>:3000
   ```
   Enable MagicDNS in Tailscale admin → DNS settings.

**Benefits:**
- No port forwarding or exposing to the internet
- Encrypted connections between devices
- Access from mobile (install Tailscale app)
- Works behind NAT/firewalls

5. **Optional: Tailscale Funnel** for public access (share with friends not on your Tailnet):
   ```bash
   # Enable HTTPS funnel on port 3000
   sudo tailscale funnel 3000
   ```
   This gives you a public `https://<hostname>.<tailnet>.ts.net` URL. Enable Funnel in Tailscale admin → Access controls first.

### Option 2: Native Rust

#### Prerequisites

- Rust 1.80+ (edition 2024)
- Tailwind CSS v4 standalone CLI (for development builds)
  - Download from: https://github.com/tailwindlabs/tailwindcss/releases
  - Place in PATH (e.g., `~/.local/bin/tailwindcss`)

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

### Cross-Compilation (Raspberry Pi)

For deploying to Raspberry Pi or other ARM64 Linux targets:

```bash
# Install cross
cargo install cross

# Build for ARM64
cross build --release --target aarch64-unknown-linux-gnu
```

The included `Cross.toml` automatically installs Tailwind CLI in the build container.

Deployment scripts are available in `scripts/`:
- `rpi-setup.sh` - Initial RPi configuration
- `rpi-deploy.sh` - Deploy binary and static assets
- `sync-audio.sh` - Sync pronunciation audio

## Configuration

Configuration via `config.toml` (copy from `config.toml.example`):

```bash
cp config.toml.example config.toml
```

### Data Directory Structure

```
data/
├── app.db               # Shared auth database (users, sessions)
└── users/
    └── <username>/
        └── learning.db  # Per-user learning database
```

Each user gets an isolated database with their own SRS state, progress, and settings.

## Authentication

The app uses a simple username/password authentication system:

- **Registration**: Create account at `/register` (username + password)
- **Login**: Authenticate at `/login`
- **Sessions**: HTTP-only cookies, 7-day expiry
- **Password Storage**: Client-side SHA-256 → server-side Argon2 (server never sees plaintext)

All routes except `/login` and `/register` require authentication.

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
├── build.rs                # Tailwind CSS build + asset hashing
├── askama.toml             # Askama configuration
├── Dockerfile              # Multi-stage Rust build
├── docker-compose.yml      # LAN deployment (kr_notebook + py-tools)
├── src/                    # Rust backend
│   ├── main.rs             # Server entry point
│   ├── lib.rs              # Module exports
│   ├── state.rs            # AppState (shared auth DB, paths)
│   ├── paths.rs            # Centralized path constants
│   ├── config.rs           # Configuration loading
│   ├── audio.rs            # Audio file handling
│   ├── session.rs          # Session ID generation
│   ├── filters.rs          # Template filters
│   ├── validation.rs       # Answer validation
│   ├── auth/               # Authentication system
│   │   ├── mod.rs          # Module exports
│   │   ├── db.rs           # Auth database (users, sessions)
│   │   ├── handlers.rs     # Login, register, logout
│   │   ├── middleware.rs   # Auth middleware, AuthContext
│   │   └── password.rs     # Argon2 hashing
│   ├── db/                 # User database layer
│   │   ├── mod.rs          # Pool management, seed data
│   │   ├── schema.rs       # Table definitions
│   │   ├── cards.rs        # Card queries
│   │   ├── reviews.rs      # Review operations
│   │   ├── stats.rs        # Statistics
│   │   └── tiers.rs        # Tier progress
│   ├── domain/             # Data models
│   ├── handlers/           # HTTP handlers
│   │   ├── mod.rs          # Index, exports
│   │   ├── study/          # Study handlers
│   │   │   ├── mod.rs
│   │   │   ├── interactive.rs  # Interactive study mode
│   │   │   ├── classic.rs      # Classic reveal-and-rate
│   │   │   ├── practice.rs     # Untracked practice
│   │   │   └── templates.rs    # Shared templates
│   │   ├── settings/       # Settings handlers
│   │   │   ├── mod.rs
│   │   │   ├── user.rs         # User settings, export/import
│   │   │   ├── admin.rs        # Admin functions, tier management
│   │   │   └── audio.rs        # Pronunciation audio config
│   │   ├── listen.rs       # Listening practice
│   │   ├── progress.rs     # Progress analytics
│   │   ├── pronunciation.rs # Audio matrix
│   │   ├── library.rs      # Character browser
│   │   ├── reference.rs    # Reference pages
│   │   └── guide.rs        # Usage guide
│   ├── profiling/          # Optional (--features profiling)
│   └── srs/                # Spaced repetition (FSRS + SM-2)
├── py_scripts/             # Python tools
│   ├── Dockerfile          # Python + ffmpeg image
│   ├── pyproject.toml
│   └── src/
│       ├── kr_scraper/     # Audio scraper
│       └── db_manager/     # Database scenarios CLI
├── templates/              # Askama HTML templates
├── doc/                    # Documentation
└── data/                   # Runtime data (gitignored)
    ├── app.db              # Shared auth database
    ├── users/<username>/   # Per-user data
    │   └── learning.db     # User's learning database
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

### Option A: Docker (py-tools service)

The `docker-compose.yml` includes a `py-tools` service with Python 3.12 and ffmpeg pre-installed:

```bash
docker compose run --rm py-tools kr-scraper lesson1
docker compose run --rm py-tools kr-scraper lesson2
docker compose run --rm py-tools kr-scraper segment
docker compose run --rm py-tools kr-scraper status
```

The `data/` directory is shared between `py-tools` and the main `kr_notebook` service.

### Option B: Native (host machine)

Requires Python 3.12+, [uv](https://docs.astral.sh/uv/), and **ffmpeg** (for audio segmentation):

```bash
# Install ffmpeg (Ubuntu/Debian)
sudo apt install ffmpeg

# Run scraper
cd py_scripts && uv sync
uv run kr-scraper lesson1
uv run kr-scraper lesson2
uv run kr-scraper segment
```

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
