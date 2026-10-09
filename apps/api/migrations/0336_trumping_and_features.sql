-- 0336_trumping_and_features.sql
-- 站型成熟度补齐 · 批次 B（质量门禁 + 能力声明机制）。
-- 来源：_doc/站型成熟度对标与达标方案-总纲v2-2026-10-09.md §5 的 G1 + §6 统一机制改造。
--
-- 两件事：
--   ① **trumping**（G1，PTP/GGn 口径）：举报劣质/死种并指定更优替代，
--      版主裁决后淘汰被举报种。承载表 `torrent_trumps`。
--      多版本分槽（G2）不建列——组内每个种子的 tier 由它自己的 `standard`
--      维度值**派生**（2160p/4K→UHD、1080p→HD、720p→SD、其余→Other），
--      派生零维护且不散 match site_type（读路径算）。
--   ② **features 能力声明**（§6 统一机制）：`site_type_packs.features JSONB`
--      ——该站型启用的垂直能力清单。消费点读 features，**不再 match site_type**，
--      这是保住「自定义站型」产品口径的唯一做法。
--
-- 全部幂等。

BEGIN;

-- ============================================================
-- ① torrent_trumps（举报 / 裁决）
-- ============================================================
CREATE TABLE IF NOT EXISTS torrent_trumps (
    id BIGSERIAL PRIMARY KEY,
    -- 被举报的种子
    torrent_id BIGINT NOT NULL REFERENCES torrents(id) ON DELETE CASCADE,
    -- 建议的替代种（可空＝只举报不指名；GGn 的 duplicates 场景会指名）
    target_torrent_id BIGINT REFERENCES torrents(id) ON DELETE SET NULL,
    reporter_id BIGINT NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    -- bad_quality（劣质）/ dead（死种）/ wrong_content（错内容）/
    -- duplicate（重复）/ other
    reason TEXT NOT NULL DEFAULT 'other',
    note TEXT NOT NULL DEFAULT '',
    -- pending / accepted / rejected
    status TEXT NOT NULL DEFAULT 'pending',
    handled_by BIGINT REFERENCES users(id) ON DELETE SET NULL,
    handled_at TIMESTAMPTZ,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now()
);

CREATE INDEX IF NOT EXISTS idx_trumps_status
    ON torrent_trumps (status, created_at DESC);
CREATE INDEX IF NOT EXISTS idx_trumps_torrent
    ON torrent_trumps (torrent_id);

COMMENT ON TABLE torrent_trumps IS
    '种子举报/替换（0336，PTP trumping + GGn duplicates 语义）：举报劣质或死种'
    '并指定更优替代，版主裁决通过后淘汰被举报种（approval_status=2）';

-- ============================================================
-- ② site_type_packs.features（能力声明）
-- ============================================================
ALTER TABLE site_type_packs
    ADD COLUMN IF NOT EXISTS features JSONB NOT NULL DEFAULT '{}'::jsonb;

COMMENT ON COLUMN site_type_packs.features IS
    '该站型启用的垂直能力清单（0336）：{"trumping":true,"quality_tiers":true,...}。'
    '消费点读本列，不 match site_type —— 自定义站型也能声明能力。';

-- 11 包默认能力面（未列出的能力默认关，向后兼容）
UPDATE site_type_packs SET features = '{"quality_tiers":true,"trumping":true}'::jsonb
 WHERE code IN ('movie');
UPDATE site_type_packs SET features = '{"quality_tiers":true}'::jsonb
 WHERE code IN ('documentary');
UPDATE site_type_packs SET features = '{"trumping":true}'::jsonb
 WHERE code IN ('game','music','lossless');
UPDATE site_type_packs SET features = '{}'::jsonb
 WHERE code IN ('general','anime','ebook','software','sports','education');

COMMIT;
