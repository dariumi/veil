# ── Build stage ──────────────────────────────────────────────────────────────
FROM rust:1.82-slim AS builder

RUN apt-get update && apt-get install -y \
    pkg-config \
    libssl-dev \
    cmake \
    && rm -rf /var/lib/apt/lists/*

WORKDIR /build

# Cache dependencies
COPY Cargo.toml Cargo.lock ./
COPY veil-core/Cargo.toml veil-core/
COPY veil-server/Cargo.toml veil-server/
COPY veil-client/Cargo.toml veil-client/

# Create dummy sources to cache deps
RUN mkdir -p veil-core/src veil-server/src veil-client/src && \
    echo "pub fn lib() {}" > veil-core/src/lib.rs && \
    echo "fn main() {}" > veil-server/src/main.rs && \
    echo "fn main() {}" > veil-client/src/main.rs

RUN cargo build --release -p veil-server 2>/dev/null || true

# Build for real
COPY veil-core/ veil-core/
COPY veil-server/ veil-server/

RUN cargo build --release -p veil-server

# ── Runtime stage ─────────────────────────────────────────────────────────────
FROM debian:bookworm-slim AS runtime

RUN apt-get update && apt-get install -y \
    ca-certificates \
    libssl3 \
    && rm -rf /var/lib/apt/lists/*

# Non-root user
RUN useradd -r -s /bin/false -d /var/lib/veil veil
RUN mkdir -p /etc/veil /var/lib/veil /var/log/veil && \
    chown -R veil:veil /etc/veil /var/lib/veil /var/log/veil

COPY --from=builder /build/target/release/veil-server /usr/local/bin/veil-server
RUN chmod +x /usr/local/bin/veil-server

# Copy default config template
COPY veil-server/config.example.toml /etc/veil/server.toml.example

# Ports: QUIC (UDP 443), TCP TLS (443), Admin API (9090)
EXPOSE 443/udp
EXPOSE 443/tcp
EXPOSE 8443/udp
EXPOSE 8443/tcp
EXPOSE 9090/tcp

VOLUME ["/etc/veil", "/var/lib/veil"]

USER veil

HEALTHCHECK --interval=30s --timeout=5s --retries=3 \
    CMD wget -q --no-check-certificate -O- https://localhost:9090/api/v1/health 2>/dev/null | grep -q '"status":"ok"' || exit 1

ENTRYPOINT ["/usr/local/bin/veil-server"]
CMD ["--config", "/etc/veil/server.toml"]
