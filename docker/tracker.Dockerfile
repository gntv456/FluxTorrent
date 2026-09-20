FROM docker.1ms.run/library/rust:1.97-slim AS builder
WORKDIR /build
COPY Cargo.toml Cargo.lock ./
COPY apps ./apps
COPY docker/cargo-config.toml /usr/local/cargo/config.toml
RUN cargo build --release -p flux-tracker

FROM docker.1ms.run/library/debian:bookworm-slim
RUN sed -i s/deb.debian.org/mirrors.tuna.tsinghua.edu.cn/g /etc/apt/sources.list.d/debian.sources 2>/dev/null; sed -i s/deb.debian.org/mirrors.tuna.tsinghua.edu.cn/g /etc/apt/sources.list 2>/dev/null; apt-get update && apt-get install -y --no-install-recommends ca-certificates && rm -rf /var/lib/apt/lists/*
# 非 root 运行（审计 P1）：专用用户 uid/gid 1000；附件卷 /app/attachments 与
# 备份卷 /backups 由 compose 卷首次创建时按此属主落盘（named volume 初次挂载会
# 继承镜像目录属主；宿主绑定目录需站长自行 chown 1000:1000）
RUN useradd --system --uid 1000 --create-home flux
COPY --from=builder /build/target/release/flux-tracker /usr/local/bin/flux-tracker
EXPOSE 7070
USER flux
ENTRYPOINT ["flux-tracker"]
