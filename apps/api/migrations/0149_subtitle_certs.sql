-- 0149 认证字幕人身份（高质量字幕翻译者标识）：
-- 自动授予（产量/评分/获奖三条件，worker 复扫）+ 后台手动授予/撤销双轨。
-- 与 medals（可摘可换装饰）分开：身份是信誉标记，随行为降级可自动撤销。

CREATE TABLE IF NOT EXISTS user_subtitle_certs (
  user_id BIGINT NOT NULL REFERENCES users(id) ON DELETE CASCADE,
  -- certified=认证字幕人 / gold=金字字幕人（获奖者，最高档）
  tier TEXT NOT NULL DEFAULT 'certified',
  source TEXT NOT NULL DEFAULT 'auto',   -- auto / admin
  reason TEXT NOT NULL DEFAULT '',
  granted_by BIGINT REFERENCES users(id), -- source=admin 时记录操作人
  granted_at TIMESTAMPTZ NOT NULL DEFAULT now(),
  revoked_at TIMESTAMPTZ,                 -- 非空 = 已撤销（保留历史）
  PRIMARY KEY (user_id, tier)
);
CREATE INDEX IF NOT EXISTS idx_usc_active ON user_subtitle_certs (user_id)
  WHERE revoked_at IS NULL;

-- 自动授予阈值（0 = 关对应条件；worker 每小时复扫，达标即授、掉标即撤）
INSERT INTO site_settings (name, value, descr) VALUES
  ('subcert_min_subtitles', '10', '认证字幕人：公开过审字幕数下限（0=不限）'),
  ('subcert_min_rating', '8.0', '认证字幕人：被评字幕的均分下限（0=不限）'),
  ('subcert_min_votes', '5', '认证字幕人：最少获评票数（0=不限）'),
  ('subcert_min_awards', '1', '金字字幕人：评选获奖次数下限（0=关该档）')
ON CONFLICT (name) DO NOTHING;

-- 触发条件注释（worker 实现口径）：
--   certified：管理员手动 或 三阈值同时满足（产量+均分+票数）
--   gold：subtitle_awards 获奖次数 ≥ subcert_min_awards
--   自动档掉标（撤销）只撤 source=auto 的行；admin 手动行只由 admin 撤
