-- 0499 渠道批续：mass_mails 圈选留痕 + 采样表保留策略 + mail 分组标签
--
-- 三件（产品体检二轮 2026-11-19）：
--   1) mass_mails 加 audience 列：dev.22 后端圈选只回显不落库——发送
--      历史看不出当时发给了哪个人群，召回/定向的运营复盘断链。
--   2) seed_contract_samples 保留策略：0496 建表后单调增长无清理
--      （结算只看上月 + 进度只看当月，>13 个月的数据零消费者）。
--      这里只加保留窗口键，worker 卫生簇按它清理。
--   3) settings 分组标签：'mail' 是 0497 首引的 grp 值，
--      meta.rs group_label/DEFAULT_GROUP_ORDER 与前端 settingGroups
--      均未登记（显示英文原名+排尾部）。本迁移不改代码，登记口径由
--      代码批统一补（见 meta.rs 与 i18n 改动）。

-- 1) 圈选留痕
ALTER TABLE mass_mails ADD COLUMN IF NOT EXISTS audience TEXT;

-- 2) 采样保留窗口（月；13 = 当月+上月+跨月缓冲，结算上限只回看 1 个月）
INSERT INTO site_settings (name, value, grp) VALUES
  ('contract_samples_retain_months', '13', 'ops')
ON CONFLICT (name) DO NOTHING;

INSERT INTO settings_meta (name, type, label_zh, label_en, hint,
                           group_key, card_order) VALUES
  ('contract_samples_retain_months', 'number', '契约采样保留月数',
   'Contract sample retention (months)',
   '契约日采样表保留窗口；结算只看上月，超过 13 个月的采样可清理',
   'ops', 9)
ON CONFLICT (name) DO NOTHING;

-- 3) 契约运营面板导航行（体检二轮 B1：admin 零读口的配套入口；
--    端点 /admin/contract-stats 权限 SETTINGS_VIEW（矩阵授 class≥90）。
--    min_class 对齐同组 economy/onboarding=98——首版误写 99，把维护
--    开发者（98）挡在门外，线上已热修，此处同步防新站复现。
INSERT INTO staff_panel_entries
    (panel, name, url, info, sort, section, min_class, tab_key, perm_key)
SELECT 'sysop', '契约运营', '/admin?tool=contractstats',
       '月度做种契约的签约/达标/罚金总览', 15, 'ops', 98,
       'contractstats', 'settings.view'
 WHERE NOT EXISTS (
     SELECT 1 FROM staff_panel_entries
     WHERE tab_key = 'contractstats'
 );
