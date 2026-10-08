ARG BASE_REGISTRY=docker.1ms.run
ARG NPM_REGISTRY=https://registry.npmmirror.com
FROM ${BASE_REGISTRY}/library/node:20-slim AS builder
WORKDIR /build
# Corepack 下载 pnpm 二进制也走镜像（默认 registry.npmjs.org 在该网络不可达）
ENV COREPACK_NPM_REGISTRY=${NPM_REGISTRY}
RUN corepack enable
# 单阶段：pnpm workspace 符号链接结构在安装与构建之间保持完整
COPY package.json pnpm-workspace.yaml pnpm-lock.yaml turbo.json ./
COPY apps/web/package.json ./apps/web/
COPY packages/domain-types/package.json ./packages/domain-types/
RUN pnpm config set registry ${NPM_REGISTRY} && pnpm install --frozen-lockfile
COPY apps/web ./apps/web
COPY packages/domain-types ./packages/domain-types
ARG NEXT_PUBLIC_API_URL=
ENV NEXT_PUBLIC_API_URL=$NEXT_PUBLIC_API_URL
# rewrites 在 build 时求值并固化 —— 构建期就要给出容器内可达的 API 地址
ARG API_SERVER_URL=http://api:8080
ENV API_SERVER_URL=$API_SERVER_URL
WORKDIR /build/apps/web
RUN pnpm exec next build

FROM ${BASE_REGISTRY}/library/node:20-slim AS runner
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
