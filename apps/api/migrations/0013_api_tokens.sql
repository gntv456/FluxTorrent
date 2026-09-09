-- M27 开放 API：第三方访问令牌（对应方案 §4.3 M27）
-- 与 JWT 会话分离：API Token 长期有效、可独立撤销、可配独立限流。
CREATE SEQUENCE IF NOT EXISTS api_tokens_id_seq;

CREATE TABLE IF NOT EXISTS api_tokens (
    id BIGINT PRIMARY KEY DEFAULT nextval('api_tokens_id_seq'),
    user_id BIGINT NOT NULL REFERENCES users(id),
    name TEXT NOT NULL,                        -- 用途备注（如 "RSS 增强客户端"）
    token_hash TEXT NOT NULL UNIQUE,           -- sha3-256(token)，明文仅签发时返回一次
    scopes TEXT[] NOT NULL DEFAULT '{read}',   -- read / torrents / economy / community
    rate_per_min INT NOT NULL DEFAULT 60,      -- 独立限流（每分钟）
    last_used_at TIMESTAMPTZ,
    revoked_at TIMESTAMPTZ,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now()
);

CREATE INDEX IF NOT EXISTS api_tokens_user_idx ON api_tokens (user_id) WHERE revoked_at IS NULL;
