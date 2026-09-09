FROM rust:1.97-slim AS builder
WORKDIR /build
# Cargo workspace 需要全部成员 manifest —— 拷贝整个 apps + 根清单
COPY Cargo.toml Cargo.lock ./
COPY apps ./apps
RUN cargo build --release -p flux-api

FROM debian:bookworm-slim
RUN apt-get update && apt-get install -y --no-install-recommends ca-certificates wget && rm -rf /var/lib/apt/lists/*
COPY --from=builder /build/target/release/flux-api /usr/local/bin/flux-api
COPY --from=builder /build/apps/api/migrations /app/migrations
WORKDIR /app
EXPOSE 8080
ENTRYPOINT ["flux-api"]
