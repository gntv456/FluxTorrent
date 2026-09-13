-- 0069 生态适配 P0/P1（2026-09-13，对照 _doc/主流PT架构横向对比与借鉴.md v2 落地清单）：
--   ① torrents.pieces_hash   —— 跨站辅种二级指纹：SHA1(info.pieces)（YemaPT piecesHash 口径）。
--      info_hash 在跨站重打包 / source 字段不同时会失效；pieces_hash 只由文件分片内容决定，
--      是辅种 / 转种生态识别"同内容"的更鲁棒指纹。
--   ② torrent_groups + torrents.group_id —— 教材/资源聚合（GZ Torrent Group 的教育域映射）：
--      同一资源的多个年份/版本/清晰度共享元数据与讨论。
--   ③ download_keys          —— 30 分钟临时下载凭证（YemaPT generateDownloadKey 口径）：
--      第三方不能拿长期 API Token 直接下载，先换短时凭证，泄露损失窗口收敛到 30 分钟。
--   ④ cheat_events           —— agent_rules 黑白名单命中落库（tracker 拒绝 → worker 落库闭环）。
--   ⑤ api_tokens.expires_at  —— API key 180 天时效（YemaPT 口径；存量 NULL 视为不限期）。

-- ② 聚合组：一个资源多版本
CREATE TABLE IF NOT EXISTS torrent_groups (
  id BIGSERIAL PRIMARY KEY,
  name TEXT NOT NULL UNIQUE,                 -- 组名（如"人教版高中数学 A 版必修一"+课程号；同名即同组，天然幂等）
  descr TEXT,                                -- 组简介（共享给组内所有版本）
  category_id INT,                           -- 组主类目（冗余，便于组内浏览）
  cover TEXT,                                -- 封面 URL（可选）
  created_by BIGINT REFERENCES users(id),
  created_at TIMESTAMPTZ NOT NULL DEFAULT now()
);

ALTER TABLE torrents ADD COLUMN IF NOT EXISTS pieces_hash CHAR(40);
ALTER TABLE torrents ADD COLUMN IF NOT EXISTS group_id BIGINT REFERENCES torrent_groups(id);
CREATE INDEX IF NOT EXISTS idx_torrents_pieces_hash ON torrents (pieces_hash) WHERE pieces_hash IS NOT NULL;
CREATE INDEX IF NOT EXISTS idx_torrents_group ON torrents (group_id) WHERE group_id IS NOT NULL;

-- ③ 临时下载凭证：明文仅签发时返回一次（fxk_ 前缀），落 sha3-256；30 分钟有效；与用户+种子绑定
CREATE TABLE IF NOT EXISTS download_keys (
  id BIGSERIAL PRIMARY KEY,
  token_hash CHAR(64) NOT NULL UNIQUE,       -- sha3-256(fxk_...)
  user_id BIGINT NOT NULL REFERENCES users(id),
  torrent_id BIGINT NOT NULL REFERENCES torrents(id),
  expires_at TIMESTAMPTZ NOT NULL,
  used_at TIMESTAMPTZ,                       -- 首次使用时刻（窗口内可复用，不做一次性）
  created_at TIMESTAMPTZ NOT NULL DEFAULT now()
);
CREATE INDEX IF NOT EXISTS idx_download_keys_user ON download_keys (user_id, created_at DESC);

-- ④ 客户端黑白名单命中记录（tracker 每 (user, agent, reason) 1 小时去重上报一次）
CREATE TABLE IF NOT EXISTS cheat_events (
  id BIGSERIAL PRIMARY KEY,
  user_id BIGINT NOT NULL,
  agent TEXT NOT NULL DEFAULT '',
  peer_ip TEXT,
  reason TEXT NOT NULL,
  hits BIGINT NOT NULL DEFAULT 1,
  first_seen TIMESTAMPTZ NOT NULL DEFAULT now(),
  last_seen TIMESTAMPTZ NOT NULL DEFAULT now(),
  UNIQUE (user_id, agent, reason)
);
CREATE INDEX IF NOT EXISTS idx_cheat_events_last ON cheat_events (last_seen DESC);

-- ⑤ API key 时效：新签发默认 180 天；require_token 校验 expires_at（NULL = 存量不限期）
ALTER TABLE api_tokens ADD COLUMN IF NOT EXISTS expires_at TIMESTAMPTZ;
