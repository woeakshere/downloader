# ---------------------------------------------------
# 1. Build Stage
# ---------------------------------------------------
# [span_3](start_span)FIX: Upgraded to 1.85-slim to support 'edition2024' dependencies[span_3](end_span)
FROM rust:1.85-slim as builder
WORKDIR /app

# Install OpenSSL headers required for 'native-tls' in Cargo.toml
RUN apt-get update && \
    apt-get install -y pkg-config libssl-dev && \
    rm -rf /var/lib/apt/lists/*

# OPTIMIZATION: Dependency Caching Layer
# We copy only manifests first and build a dummy main. 
# This caches all 90+ crate downloads so you don't re-download them every change.
COPY Cargo.toml Cargo.lock ./
RUN mkdir src && \
    echo "fn main() {println!(\"if you see this, the build failed\");}" > src/main.rs && \
    cargo build --release && \
    rm -rf src

# Now copy the actual source code
COPY . .

# Force a touch on main.rs to ensure Cargo rebuilds the actual app
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
