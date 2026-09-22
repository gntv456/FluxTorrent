# cargo-chef 分层构建（与 api.Dockerfile 同构；见该文件头注释）
FROM docker.1ms.run/library/rust:1.97-slim-bookworm AS chef
WORKDIR /build
RUN cargo install cargo-chef --locked
COPY docker/cargo-config.toml /usr/local/cargo/config.toml

FROM chef AS planner
COPY Cargo.toml Cargo.lock ./
COPY apps ./apps
RUN cargo chef prepare --recipe-path recipe.json

FROM chef AS builder
COPY --from=planner /build/recipe.json recipe.json
RUN cargo chef cook --release --recipe-path recipe.json
COPY Cargo.toml Cargo.lock ./
COPY apps ./apps
RUN cargo build --release -p flux-tracker \
    && cp target/release/flux-tracker /usr/local/bin/flux-tracker

FROM docker.1ms.run/library/debian:bookworm-slim
RUN sed -i s/deb.debian.org/mirrors.tuna.tsinghua.edu.cn/g /etc/apt/sources.list.d/debian.sources 2>/dev/null; sed -i s/deb.debian.org/mirrors.tuna.tsinghua.edu.cn/g /etc/apt/sources.list 2>/dev/null; apt-get update && apt-get install -y --no-install-recommends ca-certificates && rm -rf /var/lib/apt/lists/*
# 非 root 运行（审计 P1）：专用用户 uid/gid 1000
RUN useradd --system --uid 1000 --create-home flux
COPY --from=builder /usr/local/bin/flux-tracker /usr/local/bin/flux-tracker
EXPOSE 7070
USER flux
ENTRYPOINT ["flux-tracker"]
