FROM node:24-slim AS deps
WORKDIR /build
RUN corepack enable
COPY package.json pnpm-workspace.yaml pnpm-lock.yaml ./
COPY apps/web/package.json ./apps/web/
COPY packages/domain-types/package.json ./packages/domain-types/
RUN pnpm install --frozen-lockfile

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

FROM node:24-slim AS runner
WORKDIR /app
ENV NODE_ENV=production HOSTNAME=0.0.0.0
# Next standalone 输出自带最小 node_modules
COPY --from=builder /build/apps/web/.next/standalone ./
COPY --from=builder /build/apps/web/.next/static ./.next/static
EXPOSE 3000
CMD ["node", "apps/web/server.js"]
