# Korean Hangul Learning App

A Rust web application for learning Korean Hangul using spaced repetition (SM-2 algorithm).

## Features

- **Spaced Repetition**: SM-2 algorithm for optimal review scheduling
- **Tiered Learning**: Progress from basic to advanced characters
  - Tier 1: Basic consonants (ㄱ, ㄴ, ㄷ...) and vowels (ㅏ, ㅓ, ㅗ...)
  - Tier 2: Y-vowels (ㅑ, ㅕ...) and special ieung (ㅇ)
  - Tier 3: Aspirated (ㅋ, ㅍ...) and tense consonants (ㄲ, ㅃ...)
  - Tier 4: Compound vowels (ㅘ, ㅝ...)
- **Mobile-Friendly**: Responsive design with touch-friendly controls
- **Progress Tracking**: View your learning statistics by tier

## Tech Stack

- **Backend**: Axum (async web framework)
- **Database**: SQLite via rusqlite
- **Templating**: Askama (compile-time templates)
- **Frontend**: htmx 2.x + Tailwind CSS

## Getting Started

### Prerequisites

- Rust (1.75+)
- Cargo

### Installation

```bash
# Clone the repository
git clone <repo-url>
cd kr_notebook

# Build the project
cargo build

# Run the server
cargo run
```

The server will start at `http://localhost:3000`

### Development

```bash
# Run with hot reload (requires cargo-watch)
cargo install cargo-watch
cargo watch -x run

# Run tests
cargo test

# Check for issues
cargo clippy
```

## Usage

1. **Home** (`/`): View cards due for review and overall progress
2. **Study** (`/study`): Start a study session
   - Tap card to reveal answer
   - Rate your recall: Again, Hard, Good, or Easy
3. **Progress** (`/progress`): View detailed progress by tier

## Project Structure

```
kr_notebook/
├── Cargo.toml           # Dependencies
├── data/                # SQLite database storage
├── src/
│   ├── main.rs          # Server entry point
│   ├── lib.rs           # Module exports
│   ├── db/              # Database layer
│   ├── domain/          # Data models
│   ├── handlers/        # HTTP handlers
│   └── srs/             # SM-2 algorithm
└── templates/           # Askama HTML templates
```

## SM-2 Algorithm

The app uses the SuperMemo 2 algorithm for spaced repetition:

- **Quality ratings**: Again (0), Hard (2), Good (4), Easy (5)
- **Interval calculation**:
  - First review: 1 day
  - Second review: 6 days
  - Subsequent: previous interval × ease factor
- **Ease factor**: Adjusts based on performance (min 1.3)

## License

MIT
