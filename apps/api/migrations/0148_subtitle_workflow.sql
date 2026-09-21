-- 0148 字幕工作流与激励生态（方案 _doc/字幕工作流与激励生态策划方案.md）
-- C1 IMDB 基础 / C2 认领状态机 / C3 crew / C4 free 联动 / C5 金字幕评选

-- ============ 1) IMDB 数据基础（C1） ============
ALTER TABLE torrents ADD COLUMN IF NOT EXISTS imdb_id TEXT;
CREATE INDEX IF NOT EXISTS idx_torrents_imdb ON torrents (imdb_id) WHERE imdb_id IS NOT NULL;
ALTER TABLE subtitles ADD COLUMN IF NOT EXISTS imdb_id TEXT;

-- 历史回填：descr 里的 IMDB 链接提取（tt1234567 / tt12345678）
UPDATE torrents SET imdb_id = upper((regexp_match(descr, '(tt[0-9]{7,8})', 'i'))[1])
WHERE imdb_id IS NULL AND descr ~* 'tt[0-9]{7,8}';
UPDATE torrents SET imdb_id = upper((regexp_match(name, '(tt[0-9]{7,8})', 'i'))[1])
WHERE imdb_id IS NULL AND name ~* 'tt[0-9]{7,8}';
-- 字幕行回填（挂在有 imdb 的种子下）
UPDATE subtitles s SET imdb_id = t.imdb_id
FROM torrents t
WHERE s.torrent_id = t.id AND t.imdb_id IS NOT NULL AND s.imdb_id IS NULL;

-- ============ 2) 求字幕工作流（C2/C3/C4） ============
-- status 扩展：0 open / 1 paid / 2 dispute / 3 claimed / 4 delivering
ALTER TABLE subtitle_requests
  ADD COLUMN IF NOT EXISTS claimed_by BIGINT REFERENCES users(id),
  ADD COLUMN IF NOT EXISTS claimed_at TIMESTAMPTZ,
  ADD COLUMN IF NOT EXISTS deadline_at TIMESTAMPTZ,
  ADD COLUMN IF NOT EXISTS deliver_at TIMESTAMPTZ,
  ADD COLUMN IF NOT EXISTS offer_free BOOLEAN NOT NULL DEFAULT FALSE,
  ADD COLUMN IF NOT EXISTS free_days INT NOT NULL DEFAULT 30,
  ADD COLUMN IF NOT EXISTS crew JSONB NOT NULL DEFAULT '[]';
CREATE INDEX IF NOT EXISTS idx_subreq_claim ON subtitle_requests (claimed_by)
  WHERE status IN (3, 4);

-- ============ 3) free 奖励来源（C4） ============
ALTER TYPE promotion_source ADD VALUE IF NOT EXISTS 'subtitle';

-- ============ 4) 金字幕评选（C5） ============
CREATE TABLE IF NOT EXISTS subtitle_awards (
  id BIGSERIAL PRIMARY KEY,
  period TEXT NOT NULL,               -- '2026-10'
  subtitle_id BIGINT NOT NULL REFERENCES subtitles(id) ON DELETE CASCADE,
  user_id BIGINT NOT NULL REFERENCES users(id),
  score NUMERIC(10,2) NOT NULL,
  rank SMALLINT NOT NULL,             -- 1/2/3 获奖；0 = 入围
  tier TEXT NOT NULL DEFAULT 'human', -- human / ai 双赛道
  granted_at TIMESTAMPTZ NOT NULL DEFAULT now(),
  UNIQUE (period, subtitle_id)
);
CREATE INDEX IF NOT EXISTS idx_subawards_period ON subtitle_awards (period, rank);

-- ============ 5) 站点设定 + 子开关 ============
INSERT INTO site_settings (name, value, descr) VALUES
  ('subtitle_claim_timeout_days', '7', '求字幕认领超时（天），超时自动回池'),
  ('subtitle_crew_max', '4', '协作队伍人数上限（不含主认领人）'),
  ('subtitle_accept_timeout_days', '3', '交稿后发起人验收超时（天），超时自动验收'),
  ('subtitle_workflow', '0', '字幕工作流开关（0=NexusPHP 直交付 / 1=KG 认领流程）'),
  ('subtitle_award', '0', '金字字幕评选开关'),
  ('subtitle_award_ai_track', '1', '评选 AI 赛道开关（小站可关）'),
  ('subtitle_ai_badge', '1', 'AI 字幕角标与筛选开关')
ON CONFLICT (name) DO NOTHING;
