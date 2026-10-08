-- 0327_music_artists.sql
-- 站型成熟度 · music 艺人实体（P1 深水区的可落地半场，对标 §7.2）：
--
-- Gazelle 三层 Group → Artist → Release 中，「按艺人聚合」是最先撞上的
-- 缺口：艺人榜 / 艺人页（该艺人全部发行）/ 订阅艺人新发行都挂在它上面。
--
-- 承载评估（延续「优先轻结构」纪律）：
--   artist 是低基数字典型实体（与 torrent_groups 同量级），且我们的
--   artist 值已经存在 torrent_sections.kind='artist'（0314 text 维度）——
--   本迁移建 artists 实体表 + 上传时同步 upsert，读口聚合 sections：
--   · 上传：form artist 字段（逗号分隔多人）→ artists upsert（按名唯一）
--     → torrent_sections.artist 仍存 text（维度链路不动）；
--   · 读口：/artists（列表+搜索）、/artists/{id}（艺人页：该艺人全部
--     过审种 + 维度聚合）、艺人榜（按种子数/做种数排序）。
--   Release 层（label/catalog#/DiscID）继续由维度承载，不在本批。
--
-- 幂等：表 CREATE IF NOT EXISTS；回填按名去重。

BEGIN;

CREATE TABLE IF NOT EXISTS artists (
    id BIGSERIAL PRIMARY KEY,
    name TEXT NOT NULL UNIQUE,
    -- 名字排序键（去空白小写）：中文按拼音首字母由应用层排序，这里只存
    -- 归一化键防同义重复（"周杰伦 " 与 "周杰伦"）
    norm_name TEXT NOT NULL,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now()
);

CREATE INDEX IF NOT EXISTS idx_artists_norm ON artists (norm_name);

-- 回填：现存 torrent_sections.artist 的 text 值（拆分号/逗号多人）
INSERT INTO artists (name, norm_name)
SELECT DISTINCT TRIM(s.name), LOWER(REGEXP_REPLACE(TRIM(s.name), '\s+', '', 'g'))
FROM (
  SELECT jsonb_array_elements_text(
           CASE jsonb_typeof(ts.value)
             WHEN 'array' THEN ts.value
             ELSE jsonb_build_array(ts.value #>> '{}')
           END) AS name
  FROM torrent_sections ts
  WHERE ts.kind = 'artist'
) s
WHERE TRIM(s.name) <> ''
ON CONFLICT (name) DO NOTHING;

COMMIT;
