# ---------------------------------------------------
# 1. Build Stage
# ---------------------------------------------------
FROM rust:1.85-slim as builder
WORKDIR /app

# Install OpenSSL headers required for 'native-tls'
RUN apt-get update && \
    apt-get install -y pkg-config libssl-dev && \
    rm -rf /var/lib/apt/lists/*

# OPTIMIZATION: Dependency Caching Layer
COPY Cargo.toml ./
# We deliberately do NOT copy Cargo.lock initially to force a resolution
# compatible with our new Cargo.toml pin.
RUN mkdir src && \
    echo "fn main() {println!(\"if you see this, the build failed\");}" > src/main.rs && \
    cargo build --release && \
    rm -rf src

# Now copy the actual source code
COPY . .

# Force a rebuild of the main binary
RUN touch src/main.rs && cargo build --release

# ---------------------------------------------------
# 2. Runtime Stage
# ---------------------------------------------------
FROM debian:bookworm-slim
WORKDIR /app

# Install runtime SSL libraries
RUN apt-get update && \
    apt-get install -y ca-certificates libssl3 && \
    update-ca-certificates && \
    rm -rf /var/lib/apt/lists/*

# Copy the compiled binary
COPY --from=builder /app/target/release/leech-core /app/server

# FIX: Copy the extraction rules so extractor.rs can find them at runtime
COPY --from=builder /app/extraction_rules.json /app/extraction_rules.json

ENV PORT=8000
EXPOSE 8000

CMD ["/app/server"]
