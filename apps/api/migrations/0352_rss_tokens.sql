-- 0352 RSS token 管理（可命名/轮换/撤销，保留 passkey URL 兼容）：
--   ① rss_tokens 表：用户可自助创建多个命名 RSS 订阅 token（fxr_ 前缀 +
--      32 位随机，落库仅 sha3-256 哈希，明文只在创建/轮换响应里出现一次）。
--   ② 动机：passkey 烤进每一个 .torrent 的 announce 与既有订阅 URL，rotate
--      等于全量换种；命名 token 泄露只需单独撤销，与主 passkey 解耦。
--   ③ 兼容：/rss/{passkey} 旧 URL 永远有效——消费点先按 sha3(token) 查本表，
--      miss 再回落 user_by_passkey（先例：0069 download_keys 的哈希存储口径）。
CREATE TABLE rss_tokens (
  id BIGSERIAL PRIMARY KEY,
  user_id BIGINT NOT NULL REFERENCES users(id) ON DELETE CASCADE,
  name TEXT NOT NULL,
  token_hash CHAR(64) NOT NULL UNIQUE,     -- sha3-256(fxr_...)
  created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
  last_used_at TIMESTAMPTZ,
  revoked_at TIMESTAMPTZ
);
CREATE INDEX idx_rss_tokens_user ON rss_tokens(user_id) WHERE revoked_at IS NULL;
-- 每人上限 5 个有效 token：应用层校验（tokens.rs 创建/轮换前 COUNT），
-- 不加触发器——上限属产品策略，随版本可调，不锁进 schema。
