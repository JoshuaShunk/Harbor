# Harbor Relay Server — Multi-stage Docker build
# Runs the Harbor relay server for proxying remote MCP requests.

FROM rust:latest AS builder

WORKDIR /build

# Copy workspace files
COPY Cargo.toml Cargo.lock ./
COPY crates/ crates/
COPY ui/ ui/

# Build only the CLI binary (which includes the relay command)
RUN cargo build --release -p harbor-cli

# Runtime stage — minimal image
FROM debian:bookworm-slim

RUN apt-get update && apt-get install -y \
    ca-certificates \
    && rm -rf /var/lib/apt/lists/*

COPY --from=builder /build/target/release/harbor /usr/local/bin/harbor

# QUIC (UDP) + HTTPS (TCP)
EXPOSE 7800/udp
EXPOSE 8443/tcp

# Health check via the HTTPS frontend
HEALTHCHECK --interval=30s --timeout=5s --start-period=10s --retries=3 \
    CMD curl -sf http://localhost:8443/health || exit 1

ENTRYPOINT ["harbor", "relay"]
CMD ["--quic-port", "7800", "--https-port", "8443"]
