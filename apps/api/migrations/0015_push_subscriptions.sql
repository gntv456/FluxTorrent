-- M26 Web Push：推送订阅（方案 §4.3 M26：订阅-投递-退订闭环，用户可订阅类型）
CREATE SEQUENCE IF NOT EXISTS push_subscriptions_id_seq;

CREATE TABLE IF NOT EXISTS push_subscriptions (
    id BIGINT PRIMARY KEY DEFAULT nextval('push_subscriptions_id_seq'),
    user_id BIGINT NOT NULL REFERENCES users(id),
    endpoint TEXT NOT NULL UNIQUE,            -- 推送服务 URL（每订阅唯一）
    p256dh TEXT NOT NULL,                     -- 客户端公钥（base64url）
    auth TEXT NOT NULL,                       -- 认证密钥（base64url）
    topics TEXT[] NOT NULL DEFAULT '{promo,request,message}',
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    -- 端点失效（404/410）时标记清理
    expired_at TIMESTAMPTZ
);

CREATE INDEX IF NOT EXISTS push_subscriptions_user_idx ON push_subscriptions (user_id) WHERE expired_at IS NULL;
