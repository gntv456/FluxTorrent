FROM node:20-slim AS builder
WORKDIR /build
# Corepack 下载 pnpm 二进制也走镜像（默认 registry.npmjs.org 在该网络不可达）
ENV COREPACK_NPM_REGISTRY=https://registry.npmmirror.com
RUN corepack enable
# 单阶段：pnpm workspace 符号链接结构在安装与构建之间保持完整
COPY package.json pnpm-workspace.yaml pnpm-lock.yaml turbo.json ./
COPY apps/web/package.json ./apps/web/
COPY packages/domain-types/package.json ./packages/domain-types/
RUN pnpm config set registry https://registry.npmmirror.com && pnpm install --frozen-lockfile
COPY apps/web ./apps/web
COPY packages/domain-types ./packages/domain-types
ARG NEXT_PUBLIC_API_URL=http://localhost:8080
ENV NEXT_PUBLIC_API_URL=$NEXT_PUBLIC_API_URL
WORKDIR /build/apps/web
RUN pnpm exec next build

FROM node:20-slim AS runner
WORKDIR /app
ENV NODE_ENV=production HOSTNAME=0.0.0.0
# Next standalone 输出自带最小 node_modules
COPY --from=builder /build/apps/web/.next/standalone ./
COPY --from=builder /build/apps/web/.next/static ./.next/static
COPY --from=builder /build/apps/web/public ./apps/web/public
EXPOSE 3000
CMD ["node", "apps/web/server.js"]
