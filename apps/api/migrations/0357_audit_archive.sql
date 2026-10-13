-- 0357 审计日志保留策略：audit_log 超期行归档 + 哈希链断点锚。
--
-- 现状：audit_log 自 0266 起为防篡改哈希链（BEFORE INSERT/UPDATE/DELETE
--   触发器，DELETE/UPDATE 一律 RAISE EXCEPTION），只增不删——大站跑两
--   年后全量在线，查询与验证都退化；但直接 DELETE 会被触发器拦死。
-- 处理：三件套。
--   ① audit_log_archive：LIKE 同构列（含约束/缺省），无链触发器——归档行
--     落此表（归档动作本身由 worker 在事务内临时 DROP 触发器完成，先例
--     0266 存量回填三段式）；
--   ② site_settings.audit_retain_days（缺省 180，0=永久不归档）：worker
--     audit_retain job 每小时检查、每天最多归档一批（Redis 键节流）；
--   ③ audit_chain_anchors：每次归档记断点（cutoff 行的 self_hash）——
--     归档后存活首行的 prev_hash 不再对照全零创世，改对照最近锚。
--
-- ⚠️ settings_meta.name 有 FK → site_settings.name：先设置行后元数据行
--   （0230 首版顺序颠倒曾致启动失败）。
-- 幂等（IF NOT EXISTS / ON CONFLICT DO NOTHING），重跑安全。

-- ① 归档表：INCLUDING DEFAULTS INCLUDING CONSTRAINTS 复制列定义与
--    NOT NULL/CHECK（无 INCLUDING INDEXES → 不带源表 PK，单独补）；
--    不复制触发器（LIKE 本就不含），归档行可自由写入。
CREATE TABLE IF NOT EXISTS audit_log_archive (LIKE audit_log
    INCLUDING DEFAULTS INCLUDING CONSTRAINTS);
ALTER TABLE audit_log_archive
    ADD CONSTRAINT audit_log_archive_pkey PRIMARY KEY (id);
CREATE INDEX IF NOT EXISTS idx_audit_archive_created
    ON audit_log_archive (created_at);

-- ② 保留期设置（security 组；0=永久关闭归档，兼容现状语义）
INSERT INTO site_settings (name, value, descr, grp)
VALUES ('audit_retain_days', '180',
        '审计日志保留天数（0=永久不归档；超期行移入 audit_log_archive，'
        '哈希链断点记录于 audit_chain_anchors）',
        'security')
ON CONFLICT (name) DO NOTHING;

INSERT INTO settings_meta
  (name, type, label_zh, label_en, hint, unit, min, max, step,
   group_key, card_order, visible)
VALUES
  ('audit_retain_days', 'number', '审计日志保留天数',
   'Audit log retention (days)',
   '超期审计行每日批量移入归档表（防篡改链以锚点衔接，验证端点自动对照）；'
   '0=永久保留。归档后按月统计的历史绩效口径见 jixiao 结算注释。',
   '天', 0, 3650, 1, 'security', 10, true)
ON CONFLICT (name) DO NOTHING;

-- ③ 归档锚点：cutoff_self_hash = 删除前 cutoff 行的 self_hash。
--    chain-verify 对「最小存活 id」行：若最新锚 cutoff_id < min(alive.id)
--    则 prev_hash 应等于该锚 hash（而非全零创世）。
CREATE TABLE IF NOT EXISTS audit_chain_anchors (
    id BIGSERIAL PRIMARY KEY,
    cutoff_id BIGINT NOT NULL,
    cutoff_self_hash BYTEA NOT NULL,
    archived_rows BIGINT NOT NULL,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now()
);
