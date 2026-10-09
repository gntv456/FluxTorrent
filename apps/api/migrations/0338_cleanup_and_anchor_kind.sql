-- 0338_cleanup_and_anchor_kind.sql
-- 站型成熟度补齐 · 批次 D（死列处置 G8 + 聚合锚点通用化 G10/G11）。
-- 来源：_doc/站型成熟度对标与达标方案-总纲v2-2026-10-09.md §5。
--
-- ① G8 `torrents.technical_info` 死列：自 0001 建表起存在，全仓（Rust/TS/
--    迁移/脚本）**零读写**（仅 0001 建表行与 0019 的一条注释提到它）。留悬念
--    不如删掉——现由「MediaInfo 折叠块 + sections 维度 + torrent_artifacts」
--    三处承载技术信息。
-- ② G10/G11 把 `artists` 从「音乐专用锚点」升为**通用自由值锚点表**：
--    加 `kind` 列（artist / author / …），唯一键从 (name) 改为 (kind, name)。
--    这样 ebook 的 `author`、anime 的 `studio` 等 text 维度可复用同一套
--    聚合页/榜单/订阅面，不必每型建表（沿用 0330 的「先试复用」纪律）。
--
-- 幂等：DROP ... IF EXISTS / ADD COLUMN IF NOT EXISTS / 约束重建前先判存在。

BEGIN;

-- ============================================================
-- ① 死列处置（G8）
-- ============================================================
ALTER TABLE torrents DROP COLUMN IF EXISTS technical_info;

-- ============================================================
-- ② artists → 通用自由值锚点（kind 维度）
-- ============================================================
ALTER TABLE artists
    ADD COLUMN IF NOT EXISTS kind TEXT NOT NULL DEFAULT 'artist';

COMMENT ON COLUMN artists.kind IS
    '锚点类型（0338）：artist（音乐）/ author（电子书）/ studio（动漫）…。'
    '同一 name 在不同 kind 下是不同实体，故唯一键为 (kind, name)。';

-- 旧的 (name) 唯一约束/索引要换成 (kind, name)
ALTER TABLE artists DROP CONSTRAINT IF EXISTS artists_name_key;
DROP INDEX IF EXISTS artists_name_key;

CREATE UNIQUE INDEX IF NOT EXISTS idx_artists_kind_name
    ON artists (kind, name);
CREATE INDEX IF NOT EXISTS idx_artists_kind_norm
    ON artists (kind, norm_name);

COMMIT;
