FROM rust:1.97-slim AS builder
WORKDIR /build
COPY Cargo.toml Cargo.lock ./
COPY apps ./apps
RUN cargo build --release -p flux-worker

FROM debian:bookworm-slim
RUN apt-get update && apt-get install -y --no-install-recommends ca-certificates && rm -rf /var/lib/apt/lists/*
COPY --from=builder /build/target/release/flux-worker /usr/local/bin/flux-worker
WORKDIR /app
ENTRYPOINT ["flux-worker"]
