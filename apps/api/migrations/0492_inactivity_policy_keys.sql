-- 0492：账号保留策略配置键——公示口径（0135 FAQ/规则）与 dormant_mark
-- 实现对齐（2026-11-17 站长拍板方向 A：代码向文案看齐）。
--
-- 背景：0135 公示「普通账号连续 150 天未登录可能被回收；无任何流量的
-- 账号回收期更短；回收前邮件+站内信提醒」，但 0072 建的 dormant_mark
-- 是硬编码 90 天、无流量维度、无预提醒。本迁移补三键（管理后台可热调）：
--   inactivity_days               一般不活跃阈值（缺省 150，对齐公示）
--   inactivity_zero_traffic_days  零流量账号更短阈值（缺省 90，对齐「更短」）
--   inactivity_warn_days          打标前 N 天预提醒（缺省 14，对齐「提醒」承诺）
--
-- 口径注记：
-- · 豁免族不新增键——dormant_mark 的 class<90 条件本就豁免职员档；
--   贵宾 20/养老组 21 与捐赠者、在做种者、契约者的豁免在任务 SQL 内
--   补齐（0056 累进语义：20/21 属身份档，保留策略上视同「高等级账号」）。
-- · 「回收」动作仍走既有 inactivity_policy 三档（mark/demote/archive），
--   不做物理删号；archive 档即公示语义的「回收」（档案隐藏+登录拦截）。
INSERT INTO site_settings (name, value, grp) VALUES
  ('inactivity_days', '150', 'ops'),
  ('inactivity_zero_traffic_days', '90', 'ops'),
  ('inactivity_warn_days', '14', 'ops')
ON CONFLICT (name) DO NOTHING;

-- settings_meta 登记（后台「运维参数」区可调；label/descr 即站长手册）
INSERT INTO settings_meta (name, type, label_zh, label_en, hint,
                           group_key, card_order) VALUES
('inactivity_days', 'number', '不活跃阈值（天）', 'Inactivity threshold (days)',
 '连续 N 天未登录且无在做种的账号进入休眠流程（公示 150 天）', 'ops', 65),
('inactivity_zero_traffic_days', 'number', '零流量账号阈值（天）',
 'Zero-traffic threshold (days)',
 '上传下载均为 0 的账号适用更短阈值（公示「回收期更短」）', 'ops', 66),
('inactivity_warn_days', 'number', '休眠预提醒（天）', 'Dormancy warning (days)',
 '距休眠阈值 N 天时发邮件+站内信提醒（公示「回收前提醒」承诺）', 'ops', 67)
ON CONFLICT (name) DO NOTHING;
