# 多阶段构建（方案 §8.4：lint→build→镜像）
FROM rust:1.97-slim AS builder
WORKDIR /build
COPY Cargo.toml Cargo.lock ./
COPY apps/api ./apps/api
RUN cargo build --release -p flux-api

FROM debian:bookworm-slim
RUN apt-get update && apt-get install -y --no-install-recommends ca-certificates wget && rm -rf /var/lib/apt/lists/*
COPY --from=builder /build/target/release/flux-api /usr/local/bin/flux-api
WORKDIR /app
EXPOSE 8080
ENTRYPOINT ["flux-api"]
