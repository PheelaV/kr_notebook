FROM python:3.12-slim-bookworm

# Install ffmpeg for pydub audio processing
RUN apt-get update && apt-get install -y --no-install-recommends \
    ffmpeg \
    && rm -rf /var/lib/apt/lists/*

# Install uv for fast Python package management
COPY --from=ghcr.io/astral-sh/uv:latest /uv /usr/local/bin/uv

# Disable uv cache at runtime (dependencies are pre-installed)
ENV UV_NO_CACHE=1

WORKDIR /app

# Copy Cargo.toml for pyrootutils project root detection
COPY Cargo.toml ./

WORKDIR /app/py_scripts

# Copy project files
COPY py_scripts/pyproject.toml py_scripts/uv.lock py_scripts/README.md ./
COPY py_scripts/src/ ./src/

# Install dependencies
RUN uv sync --frozen

# Default entrypoint - run commands via uv
ENTRYPOINT ["uv", "run"]
CMD ["kr-scraper", "--help"]
