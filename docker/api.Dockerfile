# cargo-chef 分层构建（六维强化方案 阶段三「构建提速」）：
# planner 层产出 recipe.json（依赖清单指纹），cook 层只编依赖——
# 改业务代码时 recipe 不变、依赖层直接命中缓存，不再全量重编译。
# 三个 Rust 服务共用同一分层结构，仅 build 目标与运行时差异不同。
FROM docker.1ms.run/library/rust:1.97-slim-bookworm AS chef
WORKDIR /build
RUN cargo install cargo-chef --locked
COPY docker/cargo-config.toml /usr/local/cargo/config.toml

FROM chef AS planner
# Cargo workspace 需要全部成员 manifest —— 拷贝整个 apps + 根清单
COPY Cargo.toml Cargo.lock ./
COPY apps ./apps
RUN cargo chef prepare --recipe-path recipe.json

FROM chef AS builder
COPY --from=planner /build/recipe.json recipe.json
# 依赖层：recipe 未变（Cargo.toml/lock 不动）时命中缓存，跳过依赖编译
RUN cargo chef cook --release --recipe-path recipe.json
COPY Cargo.toml Cargo.lock ./
COPY apps ./apps
RUN cargo build --release -p flux-api \
    && cp target/release/flux-api /usr/local/bin/flux-api
# GeoLite2 离线库不入仓库（.gitignore）；CI 上下文无此目录时补空目录，
# 保证下方 COPY 可复现——空目录 = geo.rs 找不到 mmdb，GeoIP 字段静默降级为 null
RUN mkdir -p apps/api/geoip

FROM docker.1ms.run/library/debian:bookworm-slim
RUN sed -i s/deb.debian.org/mirrors.tuna.tsinghua.edu.cn/g /etc/apt/sources.list.d/debian.sources 2>/dev/null; sed -i s/deb.debian.org/mirrors.tuna.tsinghua.edu.cn/g /etc/apt/sources.list 2>/dev/null; apt-get update && apt-get install -y --no-install-recommends ca-certificates wget && rm -rf /var/lib/apt/lists/*
# 非 root 运行（审计 P1）：专用用户 uid/gid 1000；附件卷 /app/attachments 与
# 备份卷 /backups 由 compose 卷首次创建时按此属主落盘（named volume 初次挂载会
# 继承镜像目录属主；宿主绑定目录需站长自行 chown 1000:1000）
RUN useradd --system --uid 1000 --create-home flux
COPY --from=builder /usr/local/bin/flux-api /usr/local/bin/flux-api
COPY --from=builder /build/apps/api/migrations /app/migrations
# GeoLite2 离线库（apps/api/geoip，.gitignore 不入库；缺失时 GeoIP 字段为 null 静默降级）
COPY --from=builder /build/apps/api/geoip /app/geoip
WORKDIR /app
RUN mkdir -p /app/attachments /backups && chown -R flux:flux /app
USER flux
EXPOSE 8080
ENTRYPOINT ["flux-api"]
