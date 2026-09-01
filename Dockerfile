# ---- Build stage ------------------------------------------------------------
FROM rust:1-slim-bookworm AS builder

# `mysql` crate links native-tls (OpenSSL); utoipa-swagger-ui's build script
# fetches the Swagger UI assets over HTTPS.
RUN apt-get update && apt-get install -y --no-install-recommends \
        pkg-config libssl-dev ca-certificates \
    && rm -rf /var/lib/apt/lists/*

WORKDIR /app

# 1. Build just the dependencies against stub sources so this layer stays
#    cached until Cargo.toml / Cargo.lock change.
COPY Cargo.toml Cargo.lock ./
RUN mkdir -p src src/bin \
    && echo 'fn main() {}' > src/main.rs \
    && echo '' > src/lib.rs \
    && echo 'fn main() {}' > src/bin/gen_openapi.rs \
    && cargo build --release --bin logistics-system \
    && rm -rf src

# 2. Build the real binary.
COPY src ./src
RUN touch src/main.rs src/lib.rs \
    && cargo build --release --bin logistics-system \
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
