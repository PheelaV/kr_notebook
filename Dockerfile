# Build stage
FROM rust:slim-bookworm AS builder

WORKDIR /app

# Install build dependencies
RUN apt-get update && apt-get install -y \
    pkg-config \
    && rm -rf /var/lib/apt/lists/*

# Copy everything needed for the build
COPY Cargo.toml Cargo.lock build.rs ./
COPY src ./src
COPY templates ./templates
COPY static ./static

# Build the application
RUN cargo build --release

# Runtime stage
FROM debian:bookworm-slim

WORKDIR /app

# Install runtime dependencies
RUN apt-get update && apt-get install -y \
    ca-certificates \
    && rm -rf /var/lib/apt/lists/*

# Create data directory
RUN mkdir -p /app/data

# Copy the binary from builder
COPY --from=builder /app/target/release/kr_notebook /app/kr_notebook

# Copy static assets for runtime serving
COPY --from=builder /app/static /app/static

EXPOSE 3000

ENV RUST_LOG=kr_notebook=info,tower_http=info

CMD ["/app/kr_notebook"]
