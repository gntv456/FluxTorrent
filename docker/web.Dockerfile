FROM node:24-slim AS deps
WORKDIR /build
RUN corepack enable
COPY package.json pnpm-workspace.yaml ./
COPY apps/web/package.json ./apps/web/
COPY packages/domain-types/package.json ./packages/domain-types/
RUN pnpm install --frozen-lockfile=false

FROM node:24-slim AS builder
WORKDIR /build
RUN corepack enable
COPY --from=deps /build/node_modules ./node_modules
COPY --from=deps /build/apps/web/node_modules ./apps/web/node_modules
COPY package.json pnpm-workspace.yaml turbo.json ./
COPY apps/web ./apps/web
COPY packages/domain-types ./packages/domain-types
ARG NEXT_PUBLIC_API_URL=http://localhost:8080
ENV NEXT_PUBLIC_API_URL=$NEXT_PUBLIC_API_URL
WORKDIR /build/apps/web
RUN pnpm exec next build

FROM node:24-slim
WORKDIR /app
ENV NODE_ENV=production
COPY --from=builder /build/apps/web/.next ./.next
COPY --from=builder /build/apps/web/public ./public
COPY --from=builder /build/apps/web/package.json ./package.json
COPY --from=builder /build/apps/web/next.config.ts ./next.config.ts
EXPOSE 3000
CMD ["npx", "next", "start", "-p", "3000"]
