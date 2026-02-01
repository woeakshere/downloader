# Build Stage
# Upgraded to 1.85 to support Edition 2024 dependencies like time-macros
FROM rust:1.85-slim as builder
WORKDIR /app

# Install OpenSSL headers for native-tls compilation
RUN apt-get update && \
    apt-get install -y pkg-config libssl-dev && \
    rm -rf /var/lib/apt/lists/*

# Optimization: Pre-fetch and build dependencies to cache this layer
# This prevents re-downloading everything if only your source code changes
COPY Cargo.toml Cargo.lock ./
RUN mkdir src && echo "fn main() {}" > src/main.rs && \
    cargo build --release && \
    rm -rf src/

# Now copy the actual source code and extraction rules
COPY . .

# Build the real binary
RUN cargo build --release

# Runtime Stage
FROM debian:bookworm-slim
WORKDIR /app

# Install runtime OpenSSL libraries and certificates
RUN apt-get update && \
    apt-get install -y ca-certificates libssl3 && \
    update-ca-certificates && \
    rm -rf /var/lib/apt/lists/*

# Copy the binary from the builder stage
COPY --from=builder /app/target/release/leech-core /app/server
# Ensure the JSON rules are available at runtime for the LinkExtractor
COPY --from=builder /app/extraction_rules.json /app/extraction_rules.json

ENV PORT=8000
EXPOSE 8000

CMD ["/app/server"]
