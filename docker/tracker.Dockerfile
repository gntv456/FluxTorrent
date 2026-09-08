FROM rust:1.97-slim AS builder
WORKDIR /build
COPY Cargo.toml Cargo.lock ./
COPY apps/api ./apps/api
COPY apps/worker ./apps/worker
COPY apps/tracker ./apps/tracker
RUN cargo build --release -p flux-tracker

FROM debian:bookworm-slim
RUN apt-get update && apt-get install -y --no-install-recommends ca-certificates && rm -rf /var/lib/apt/lists/*
COPY --from=builder /build/target/release/flux-tracker /usr/local/bin/flux-tracker
EXPOSE 7070
ENTRYPOINT ["flux-tracker"]
