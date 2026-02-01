# Build Stage
FROM rust:1.83-slim as builder
WORKDIR /app

# Install OpenSSL headers for native-tls compilation
RUN apt-get update && \
    apt-get install -y pkg-config libssl-dev && \
    rm -rf /var/lib/apt/lists/*

COPY . .
RUN cargo build --release

# Runtime Stage
FROM debian:bookworm-slim
WORKDIR /app

# Install runtime OpenSSL libraries and certificates
RUN apt-get update && \
    apt-get install -y ca-certificates libssl3 && \
    update-ca-certificates && \
    rm -rf /var/lib/apt/lists/*

COPY --from=builder /app/target/release/leech-core /app/server

ENV PORT=8000
EXPOSE 8000

CMD ["/app/server"]
