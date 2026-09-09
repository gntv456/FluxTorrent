FROM docker.1ms.run/library/rust:1.97-slim AS builder
WORKDIR /build
COPY Cargo.toml Cargo.lock ./
COPY apps ./apps
COPY docker/cargo-config.toml /usr/local/cargo/config.toml
RUN cargo build --release -p flux-worker

FROM docker.1ms.run/library/debian:bookworm-slim
RUN sed -i s/deb.debian.org/mirrors.tuna.tsinghua.edu.cn/g /etc/apt/sources.list.d/debian.sources 2>/dev/null; sed -i s/deb.debian.org/mirrors.tuna.tsinghua.edu.cn/g /etc/apt/sources.list 2>/dev/null; apt-get update && apt-get install -y --no-install-recommends ca-certificates && rm -rf /var/lib/apt/lists/*
COPY --from=builder /build/target/release/flux-worker /usr/local/bin/flux-worker
WORKDIR /app
ENTRYPOINT ["flux-worker"]
