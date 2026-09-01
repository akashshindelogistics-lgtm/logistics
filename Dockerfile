# ---- Build stage ------------------------------------------------------------
# Full (not -slim) image: it already carries gcc, pkg-config, libssl-dev, git
# and curl, which the `mysql` (native-tls / OpenSSL) and `utoipa-swagger-ui`
# crates need to build.
FROM rust:1-bookworm AS builder

# Be forgiving of flaky registry/CDN transfers in CI, and keep peak memory
# down on the 4-core arm64 runners.
ENV CARGO_NET_RETRY=10 \
    CARGO_HTTP_TIMEOUT=300 \
    CARGO_INCREMENTAL=0 \
    CARGO_BUILD_JOBS=4

WORKDIR /app
COPY Cargo.toml Cargo.lock ./
COPY src ./src

# Fetch first (network-only, retried) so a transient download failure is a
# clear, isolated error rather than an opaque `cargo build` exit 101. Swagger
# UI is vendored (Cargo.toml), so this is the only network the build needs.
RUN cargo fetch --locked
RUN cargo build --release --locked --bin logistics-system \
    && cp target/release/logistics-system /usr/local/bin/logistics-system

# ---- Runtime stage ---------------------------------------------------------
FROM debian:bookworm-slim AS runtime

RUN apt-get update && apt-get install -y --no-install-recommends \
        ca-certificates libssl3 curl \
    && rm -rf /var/lib/apt/lists/* \
    && useradd -r -u 10001 app

COPY --from=builder /usr/local/bin/logistics-system /usr/local/bin/logistics-system

USER app
ENV HOST=0.0.0.0 PORT=8080
EXPOSE 8080

HEALTHCHECK --interval=30s --timeout=5s --start-period=20s --retries=3 \
    CMD curl -sf http://127.0.0.1:8080/api/health || exit 1

CMD ["logistics-system"]
