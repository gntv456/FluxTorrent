-- 0036 请求补种 + 客户端黑白名单 + 标签字典（NP takereseed / AgentAllow / tags 口径）
-- T-03：请求补种限频列（900 秒内不重复群发，NP takereseed.php）
ALTER TABLE torrents ADD COLUMN IF NOT EXISTS last_reseed TIMESTAMPTZ;

-- G-06：客户端（peer agent）黑白名单（NP agent_allowed / agent_deny 口径；mode: allow 白名单优先 / deny 黑名单）
CREATE TABLE IF NOT EXISTS agent_rules (
    id          BIGSERIAL PRIMARY KEY,
    mode        TEXT NOT NULL CHECK (mode IN ('allow', 'deny')),
    pattern     TEXT NOT NULL,                    -- 前缀或子串匹配，如 "Transmission/3"
    note        TEXT,
    created_by  BIGINT REFERENCES users(id),
    created_at  TIMESTAMPTZ NOT NULL DEFAULT now()
);
CREATE INDEX IF NOT EXISTS idx_agent_rules_mode ON agent_rules (mode);

-- T-04：标签字典（tags 表已有 torrent_id/tag_id 关联，补字典表与官种=3 口径）
CREATE TABLE IF NOT EXISTS tag_dict (
    id    INT PRIMARY KEY,
    name  TEXT NOT NULL UNIQUE,
    kind  TEXT NOT NULL DEFAULT 'plain'           -- plain / official
);
INSERT INTO tag_dict (id, name, kind) VALUES
  (1, '官方', 'official'),
  (2, '免费', 'plain'),
  (3, '官种', 'official'),
  (4, '合集', 'plain'),
  (5, '带答案', 'plain')
ON CONFLICT (id) DO NOTHING;
