-- 0285：PT 经济不变式 + 凭证处置权限（2026-10-05 版主实测审计 P0/P0/P1 的落地面）
--
-- 背景（实测复现，报告 _doc/PT版主视角实测审计-2026-10-05.md）：
--   P0-1 announce 计数器回放：客户端累计总量「报 500GB → 报 0 → 再报 500GB」可重复入账，
--        5 次 announce 从一个 15MB 种子刷出 2.5TB。防线＝事件恰好入账一次（announce_seen）
--        ＋ 增量按「自上次上报的真实秒数 × 物理速率上限」钳制（超限部分不入账、留痕待核）。
--   P0-2 管理端补流量蒸发：users.uploaded 被直改而不写流水，下一次 announce 的重算即抹掉。
--        流水列 promotion_kind 之外新增 reason，让 admin 调整有出处可回溯。
--   P0-3 passkey 明文：后台用户详情返回全员明文 passkey 且不写审计。新增 reveal 权限键，
--        与「重置他人 passkey」一起收归主管/站长。
--
-- ⚠️ 值行先于登记行（settings_meta.name 有 FK → site_settings.name，0230 教训）。
-- ⚠️ 全部幂等（IF NOT EXISTS / ON CONFLICT DO NOTHING），撞号或重跑不炸 api。

-- ① 事件恰好入账一次：XADD 流 id 作为幂等键（重投/PEL 回收不再双计做种时长与流量）
CREATE TABLE IF NOT EXISTS announce_seen (
    event_id text PRIMARY KEY,
    user_id  bigint NOT NULL,
    seen_at  timestamptz NOT NULL DEFAULT now()
);
CREATE INDEX IF NOT EXISTS idx_announce_seen_seen_at ON announce_seen (seen_at);

-- ② 流量流水：带上事件 id / 调整理由 / 操作者，让每一笔入账都能回溯到出处。
--    注：traffic_ledger 是按 window_start 分区的表，PG 不允许建不含分区键的唯一索引，
--    所以「恰好入账一次」由上方 announce_seen 的主键承担（同事务内先占位），
--    本表 event_id 只作追溯与对账用。
ALTER TABLE traffic_ledger ADD COLUMN IF NOT EXISTS event_id text;
ALTER TABLE traffic_ledger ADD COLUMN IF NOT EXISTS reason text;
ALTER TABLE traffic_ledger ADD COLUMN IF NOT EXISTS operator_id bigint;
-- 管理端补量/发放也走这张流水（否则被 announce_main 的「基线+流水」重算覆盖），
-- 而这类调整不绑定具体种子 ⇒ torrent_id 放开可空。
-- 上游消费方（maintain 汇总 / jixiao 月度 / audit 抽查）均按 user_id 聚合，NULL 不影响。
ALTER TABLE traffic_ledger ALTER COLUMN torrent_id DROP NOT NULL;
CREATE INDEX IF NOT EXISTS idx_traffic_ledger_event ON traffic_ledger (event_id)
    WHERE event_id IS NOT NULL;
CREATE INDEX IF NOT EXISTS idx_traffic_ledger_operator ON traffic_ledger (operator_id)
    WHERE operator_id IS NOT NULL;

-- ③ 站点设定：可入账的物理速率上限（复用 anticheat 组的 speed_alarm_bps 语义——
--    超过它即视为物理不可能，超出部分不自动入账，改记 cheat_events 等管理组裁定）。
--    同时登记一个独立键，让「告警阈值」与「入账上限」可以分开设（上限可严于告警线）。
INSERT INTO site_settings (name, value, descr, grp)
VALUES ('traffic_credit_max_bps', '2147483648',
        '单次 announce 可入账的速率上限（字节/秒）：本次增量 ÷ 距上次上报秒数 超过该值的部分不予入账，'
        '并记入作弊事件待管理组复核。默认与 speed_alarm_bps 同值；机房高上行站点可上调，'
        '防刷站建议下调到实际可达带宽（如 1Gbps=134217728）。',
        'anticheat')
ON CONFLICT (name) DO NOTHING;

INSERT INTO settings_meta
  (name, type, label_zh, label_en, hint, group_key, card_order, visible, min, max)
VALUES
  ('traffic_credit_max_bps', 'number', '可入账速率上限',
   'Credible rate ceiling',
   'announce 增量按「距上次上报的秒数 × 本值」为上限入账，超出部分只记事件不计流量，'
   '堵死「计数器回放刷上传量」。与 speed_alarm_bps（告警线）分设：可以只告警不扣量时把本值调大。',
   'anticheat', 12, true, 1048576, 1099511627776)
ON CONFLICT (name) DO NOTHING;

-- ④ 新权限键：查看/重置他人 passkey（明文凭证属高敏动作）
INSERT INTO permissions (key, name, category, descr, sort, implemented) VALUES
('user.passkey.reveal', '查看/重置成员 passkey', 'system',
 '读取成员 passkey 明文或代为重置；留审计', 152, true)
ON CONFLICT (key) DO NOTHING;

-- 主管(95) 与站长(99) 持有；版主/管理员不可（默认最小权限）
INSERT INTO role_permissions (role_type, role_key, permission_key)
VALUES ('class', '95', 'user.passkey.reveal'), ('class', '99', 'user.passkey.reveal')
ON CONFLICT (role_type, role_key, permission_key) DO NOTHING;

-- ⑤ 审核权独立键（0285）：过去审/改/删全挤在 torrent.manage，审核台只判 staff.panel，
--   于是 91 发布员也能审种、甚至审自己的种（利益冲突）。这里先把「审」拆出来，
--   授予门槛与今天等价（class ≥ 90），站长之后可按区/按级再收——
--   自审拦截在代码侧（review_decide），与权限粒度无关。
INSERT INTO permissions (key, name, category, descr, sort, implemented) VALUES
('torrent.review', '审核种子（通过/拒绝）', 'upload',
 '在审核台对他人种子做通过或拒绝', 121, true)
ON CONFLICT (key) DO NOTHING;

INSERT INTO role_permissions (role_type, role_key, permission_key)
VALUES ('class', '90', 'torrent.review')
ON CONFLICT (role_type, role_key, permission_key) DO NOTHING;

-- ⑥ 举报去重：同一举报人对同一目标只允许一条在处理中（0=pending / 2=handling）。
--    历史重复行先折叠保留最早一条，再建部分唯一索引——否则建索引即失败。
DELETE FROM reports r USING reports k
 WHERE r.id > k.id
   AND r.ref_type = k.ref_type
   AND r.ref_id = k.ref_id
   AND r.reporter_id = k.reporter_id
   AND r.status IN (0, 2) AND k.status IN (0, 2);

CREATE UNIQUE INDEX IF NOT EXISTS uq_reports_open_per_reporter
    ON reports (ref_type, ref_id, reporter_id) WHERE status IN (0, 2);
