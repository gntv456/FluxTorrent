FROM docker.1ms.run/library/rust:1.97-slim-bookworm AS builder
WORKDIR /build
# Cargo workspace 需要全部成员 manifest —— 拷贝整个 apps + 根清单
COPY Cargo.toml Cargo.lock ./
COPY apps ./apps
COPY docker/cargo-config.toml /usr/local/cargo/config.toml
RUN cargo build --release -p flux-api

FROM docker.1ms.run/library/debian:bookworm-slim
RUN sed -i s/deb.debian.org/mirrors.tuna.tsinghua.edu.cn/g /etc/apt/sources.list.d/debian.sources 2>/dev/null; sed -i s/deb.debian.org/mirrors.tuna.tsinghua.edu.cn/g /etc/apt/sources.list 2>/dev/null; apt-get update && apt-get install -y --no-install-recommends ca-certificates wget && rm -rf /var/lib/apt/lists/*
COPY --from=builder /build/target/release/flux-api /usr/local/bin/flux-api
COPY --from=builder /build/apps/api/migrations /app/migrations
# GeoLite2 离线库（apps/api/geoip，.gitignore 不入库；缺失时 GeoIP 字段为 null 静默降级）
COPY --from=builder /build/apps/api/geoip /app/geoip
WORKDIR /app
EXPOSE 8080
ENTRYPOINT ["flux-api"]
