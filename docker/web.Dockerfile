ARG BASE_REGISTRY=docker.io
# 默认走官方 npm registry：GitHub Actions 构建机在海外，官方源最稳；此前默认
# npmmirror（国内镜像）在 CI 侧间歇性 500（corepack 拉 pnpm 本体失败 → pnpm
# install 直接 exit 1，2026-11-09 连续两版 web 构建卡在此）。国内构建可传
# --build-arg NPM_REGISTRY=https://registry.npmmirror.com 覆盖。
ARG NPM_REGISTRY=https://registry.npmjs.org
FROM ${BASE_REGISTRY}/library/node:24-slim AS builder
# 全局 ARG（FROM 前声明）在 stage 内不可见，必须在本 stage 重新声明才能
# 被 ENV/RUN 引用——否则 COREPACK_NPM_REGISTRY 落空，corepack 回落
# registry.npmjs.org，在受限网络下 pnpm install 直接失败（实测 exit 1）。
ARG NPM_REGISTRY
WORKDIR /build
# Corepack 下载 pnpm 二进制也走镜像（默认 registry.npmjs.org 在该网络不可达）
ENV COREPACK_NPM_REGISTRY=${NPM_REGISTRY}
RUN corepack enable
# 单阶段：pnpm workspace 符号链接结构在安装与构建之间保持完整
COPY package.json pnpm-workspace.yaml pnpm-lock.yaml turbo.json ./
COPY apps/web/package.json ./apps/web/
COPY packages/domain-types/package.json ./packages/domain-types/
RUN pnpm config set registry "${NPM_REGISTRY:-https://registry.npmjs.org}" \
    && pnpm install --frozen-lockfile
COPY apps/web ./apps/web
COPY packages/domain-types ./packages/domain-types
ARG NEXT_PUBLIC_API_URL=
ENV NEXT_PUBLIC_API_URL=$NEXT_PUBLIC_API_URL
# rewrites 在 build 时求值并固化 —— 构建期就要给出容器内可达的 API 地址
ARG API_SERVER_URL=http://api:8080
ENV API_SERVER_URL=$API_SERVER_URL
WORKDIR /build/apps/web
RUN pnpm exec next build
# postbuild 产物（public/i18n/dict-chunks.json 等构建期生成、gitignore 不入库）
# 必须在 builder 里显式重跑一次：pnpm exec next build 会触发 package.json
# 的 postbuild 脚本，但 CI 缓存/环境差异下仍显式保证清单存在——layout 的
# 字典 chunk preload 依赖它（缺失时静默退化，不阻断）。
RUN node ../../scripts/gen-dict-chunks.mjs || true

FROM ${BASE_REGISTRY}/library/node:24-slim AS runner
WORKDIR /app
ENV NODE_ENV=production HOSTNAME=0.0.0.0
# Next standalone 输出自带最小 node_modules
COPY --from=builder /build/apps/web/.next/standalone ./
# server.js 位于 apps/web/ 下，静态资源须相对它放置（/app/apps/web/.next/static、/app/apps/web/public）
COPY --from=builder /build/apps/web/.next/static ./apps/web/.next/static
COPY --from=builder /build/apps/web/public ./apps/web/public
# 非 root 运行（审计 P1）：node 官方 slim 镜像自带 uid 1000 node 用户
RUN chown -R node:node /app
USER node
EXPOSE 3000
CMD ["node", "apps/web/server.js"]
