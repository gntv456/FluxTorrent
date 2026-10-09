-- 0337_preview_and_log_adjust.sql
-- 站型成熟度补齐 · 批次 C（富媒体预览 G5/G9 + 日志人工改判 G3）。
-- 来源：_doc/站型成熟度对标与达标方案-总纲v2-2026-10-09.md §5。
--
-- ① 试读/试听（G5 ebook / G9 音乐）：
--    MAM 的样章试读、RED 的高保真试听，本质是「作品条目下挂一段受限片段」。
--    承载不新建存储：**物理文件复用 `attachments`**（已有 sha256 去重 + 配额 +
--    可见性），本表只做「种子 ↔ 附件」的关联与排序。
-- ② 日志人工改判（G3，Gazelle `AdjustedScore/AdjustedBy/AdjustmentReason` 口径）：
--    非英文日志、HTOA 区间等的 rescore 请求需要有承载面。原始 `log_score`
--    保留不动，改判写 `adjusted_*` 四列 ⇒ 判分结果与人工口径可同时审计。
-- ③ features 补 `preview`：能力仍走声明机制，不 match site_type。
--
-- 全部幂等。

BEGIN;

-- ============================================================
-- ① torrent_previews（种子 ↔ 预览片段）
-- ============================================================
CREATE TABLE IF NOT EXISTS torrent_previews (
    id BIGSERIAL PRIMARY KEY,
    torrent_id BIGINT NOT NULL REFERENCES torrents(id) ON DELETE CASCADE,
    -- 关联 attachments.sha256（物理文件与配额归附件体系管）
    sha256 TEXT NOT NULL,
    filename TEXT NOT NULL DEFAULT '',
    mime TEXT NOT NULL DEFAULT '',
    size_bytes BIGINT NOT NULL DEFAULT 0,
    -- preview（试读/试看图文）/ audio（试听片段）
    kind TEXT NOT NULL DEFAULT 'preview',
    sort INT NOT NULL DEFAULT 0,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    UNIQUE (torrent_id, sha256)
);

CREATE INDEX IF NOT EXISTS idx_torrent_previews_torrent
    ON torrent_previews (torrent_id, sort, id);

COMMENT ON TABLE torrent_previews IS
    '种子预览片段（0337）：试读/试听。物理文件复用 attachments（sha256 指向），'
    '本表只管种子↔附件的关联与排序。';

-- ============================================================
-- ② torrent_logs 人工改判列（G3）
-- ============================================================
ALTER TABLE torrent_logs
    ADD COLUMN IF NOT EXISTS adjusted_score SMALLINT;
ALTER TABLE torrent_logs
    ADD COLUMN IF NOT EXISTS adjusted_by BIGINT
        REFERENCES users(id) ON DELETE SET NULL;
ALTER TABLE torrent_logs
    ADD COLUMN IF NOT EXISTS adjust_reason TEXT;
ALTER TABLE torrent_logs
    ADD COLUMN IF NOT EXISTS adjusted_at TIMESTAMPTZ;

COMMENT ON COLUMN torrent_logs.adjusted_score IS
    '人工改判后的日志分（0337）：空 = 未改判，读取时以本列为准、否则用 log_score。'
    'log_score 保留原始判分，两者可同时审计。';

-- ============================================================
-- ③ features 补 preview（声明机制，不 match site_type）
-- ============================================================
UPDATE site_type_packs
   SET features = COALESCE(features, '{}'::jsonb) || '{"preview":true}'::jsonb
 WHERE code IN ('ebook', 'music', 'lossless');

COMMIT;
