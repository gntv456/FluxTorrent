ARG BASE_REGISTRY=docker.io
# cargo-chef 分层构建（六维强化方案 阶段三「构建提速」）：
# planner 层产出 recipe.json（依赖清单指纹），cook 层只编依赖——
# 改业务代码时 recipe 不变、依赖层直接命中缓存，不再全量重编译。
# 三个 Rust 服务共用同一分层结构，仅 build 目标与运行时差异不同。
FROM ${BASE_REGISTRY}/library/rust:1.97-slim-bookworm AS chef
WORKDIR /build
RUN cargo install cargo-chef --locked
# cargo 源可覆盖：默认 rsproxy 国内镜像；海外构建
# --build-arg CARGO_REGISTRY=sparse+https://index.crates.io/
ARG CARGO_REGISTRY=sparse+https://rsproxy.cn/index/
COPY docker/cargo-config.toml /tmp/cargo-config.toml
RUN sed "s|sparse+https://rsproxy.cn/index/|${CARGO_REGISTRY}|" /tmp/cargo-config.toml > /usr/local/cargo/config.toml && rm /tmp/cargo-config.toml

FROM chef AS planner
# 适配器 wasm 单独成层：adapter_seed.rs 的 include_bytes! 在编译期就要这个产物，
# 而它落在 **/target/（被 .dockerignore 排除、也没入库）——不在这里构建，
# 干净克隆的 api 镜像必然编不过（四审 A 批复验空库首启时踩实）。
# 放在 `COPY apps` 之前，业务代码改动不再连带重付这 29 分钟的 wasm 编译。
# rustup 走 rsproxy 国内镜像（与下方 cargo 的 rsproxy 同源）：
# static.rust-lang.org 直连被 GFW 限速掐断过——rustup 内部重试 80 分钟后
# stream error 回滚整次构建（2026-10-14 实测），容器无代理改走镜像。
ARG RUSTUP_DIST_SERVER=https://rsproxy.cn
ARG RUSTUP_UPDATE_ROOT=https://rsproxy.cn/rustup
COPY apps/api/adapters ./apps/api/adapters
RUN rustup target add wasm32-unknown-unknown \
    && cd apps/api/adapters/douban \
    && cargo build --release --target wasm32-unknown-unknown
# Cargo workspace 需要全部成员 manifest —— 拷贝整个 apps + 共享 crate + 根清单
COPY Cargo.toml Cargo.lock ./
COPY apps ./apps
COPY crates ./crates
RUN cargo chef prepare --recipe-path recipe.json

FROM chef AS builder
COPY --from=planner /build/recipe.json recipe.json
# 依赖层：recipe 未变（Cargo.toml/lock 不动）时命中缓存，跳过依赖编译
RUN cargo chef cook --release --recipe-path recipe.json
COPY Cargo.toml Cargo.lock ./
COPY apps ./apps
COPY crates ./crates
COPY --from=planner /build/apps/api/adapters/douban/target/wasm32-unknown-unknown/release/adapter_douban.wasm \
    /build/apps/api/adapters/douban/target/wasm32-unknown-unknown/release/adapter_douban.wasm
RUN cargo build --release -p flux-api --features douzero \
    && cp target/release/flux-api /usr/local/bin/flux-api
# GeoLite2 离线库不入仓库（.gitignore）；CI 上下文无此目录时补空目录，
# 保证下方 COPY 可复现——空目录 = geo.rs 找不到 mmdb，GeoIP 字段静默降级为 null
RUN mkdir -p apps/api/geoip
# DouZero 模型权重不入仓库（.gitignore，Apache-2.0）；构建时从 HF 镜像拉取。
# sha256 逐一核对（错一个字节即断构建），三件齐才放行——此前 `|| rm -f` 掩错
# 使失败层被缓存固化（空目录进镜像，2026-11-26 dev.48 双站实测踩坑）。
# 缺模型时运行时静默回落启发式 ai_choose，不阻塞启动。
ARG HF_MIRROR=hf-mirror.com
# 缓存钉子：改失效该层缓存时 bump（MODEL_CACHE_BUST=2 起）
ARG MODEL_CACHE_BUST=2
# rust slim 无 wget（127 not found——上一轮 ||rm 掩错把这也盖住了）
RUN apt-get update && apt-get install -y --no-install-recommends wget ca-certificates \
    && rm -rf /var/lib/apt/lists/*
RUN mkdir -p apps/api/models/doudizhu \
    && for f in landlord landlord_down landlord_up; do \
         wget -q "https://${HF_MIRROR}/palemoky/douzero-baselines/resolve/main/models_onnx/douzero_WP/$f.onnx" \
           -O "apps/api/models/doudizhu/$f.onnx"; \
       done \
    && echo "5ba2f1bb414483a3ecdf0d000159c1322b02e2d096e6d83be8a18765b56d6d32  apps/api/models/doudizhu/landlord.onnx" > /tmp/dz.sha \
    && echo "9b816457615f8007dd3a657dde1f78dac4fec7c3b0041f6f89321a1c6335cbd1  apps/api/models/doudizhu/landlord_down.onnx" >> /tmp/dz.sha \
    && echo "244ec62a549a0141c6532dffd02bee7675621d095cb194362b47c8a923c95883  apps/api/models/doudizhu/landlord_up.onnx" >> /tmp/dz.sha \
    && sha256sum -c /tmp/dz.sha \
    && rm /tmp/dz.sha \
    && ls -la apps/api/models/doudizhu

FROM ${BASE_REGISTRY}/library/debian:bookworm-slim
# apt 走官方源：GitHub runner 在海外，换国内镜像（tuna）反被墙/超时——
# v0.4.0 发布实录：arm64 job apt exit 100 即此因。国内构建想提速用
# --build-arg 换 APT_MIRROR，不写死。
ARG APT_MIRROR=deb.debian.org
# postgresql-client-16：pg 容器是 16（P-17）。bookworm 主源只有 client 15
# （pg_dump 15 连 16 服务器直接「version mismatch」），从 PGDG 官方源装 16；
# PGDG 不可达时回退主源 client（裸机部署自带匹配 pg_dump 的场景不受影响）。
RUN sed -i s/deb.debian.org/${APT_MIRROR}/g /etc/apt/sources.list.d/debian.sources 2>/dev/null; sed -i s/deb.debian.org/${APT_MIRROR}/g /etc/apt/sources.list 2>/dev/null; apt-get update \
  && apt-get install -y --no-install-recommends ca-certificates wget gnupg libstdc++6 \
  && (wget -qO- https://www.postgresql.org/media/keys/ACCC4CF8.asc | gpg --dearmor -o /usr/share/keyrings/pgdg.grep 2>/dev/null || true) \
  && echo "deb [signed-by=/usr/share/keyrings/pgdg.grep] http://apt.postgresql.org/pub/repos/apt bookworm-pgdg main" > /etc/apt/sources.list.d/pgdg.list \
  && (apt-get update && apt-get install -y --no-install-recommends postgresql-client-16 \
      || apt-get install -y --no-install-recommends postgresql-client) \
  && rm -rf /var/lib/apt/lists/*
# 非 root 运行（审计 P1）：专用用户 uid/gid 1000；附件卷 /app/attachments 与
# 备份卷 /backups 由 compose 卷首次创建时按此属主落盘（named volume 初次挂载会
# 继承镜像目录属主；宿主绑定目录需站长自行 chown 1000:1000）
RUN useradd --system --uid 1000 --create-home flux
# onnxruntime 动态库（load-dynamic 运行时 dlopen，ORT_DYLIB_PATH 缺省指向这里）。
# 源 = npm onnxruntime-node@1.31.1 内置 napi 二进制（registry.npmjs.org 全网可达；
# 官方 GitHub releases 部分网络段被屏蔽 404 实测）。ELF x86-64 动态库，符号需求
# GLIBC 2.28 / GLIBCXX 3.4.22（GCC 8.1），bookworm(2.36/12) 兼容；API 层与
# ort-sys rc.15 的 api-31 对齐。dlopen 失败 → available()=false → 回落启发式。
RUN mkdir -p /app/lib /tmp/ort_npm \
    && wget -q "https://registry.npmjs.org/onnxruntime-node/-/onnxruntime-node-1.31.1.tgz" -O /tmp/ort.tgz \
    && tar -xzf /tmp/ort.tgz -C /tmp/ort_npm \
    && cp /tmp/ort_npm/package/bin/napi-v6/linux/x64/libonnxruntime.so.1 /app/lib/libonnxruntime.so \
    && rm -rf /tmp/ort.tgz /tmp/ort_npm \
    && ls -la /app/lib
COPY --from=builder /usr/local/bin/flux-api /usr/local/bin/flux-api
COPY --from=builder /build/apps/api/migrations /app/migrations
# GeoLite2 离线库（apps/api/geoip，.gitignore 不入库；缺失时 GeoIP 字段为 null 静默降级）
COPY --from=builder /build/apps/api/geoip /app/geoip
# DouZero 模型（apps/api/models/doudizhu，构建时下载；缺失目录也容忍——
# 运行时 dz_nn::available() 探测失败自动回落启发式）
COPY --from=builder /build/apps/api/models /app/models
WORKDIR /app
RUN mkdir -p /app/attachments /backups && chown -R flux:flux /app
USER flux
EXPOSE 8080
ENTRYPOINT ["flux-api"]
