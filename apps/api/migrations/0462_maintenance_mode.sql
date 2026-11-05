-- 0462: 全站维护模式（对标 NexusPHP main.site_online / Unit3D artisan down）。
--
-- 背景（2026-11-05 站长采纳）：此前想让站点进维护只能停容器或反代兜 503，
-- 产品内没有「站长一键切维护 + 员工照常访问」的开关；offline 页的
-- 「预计恢复时间」挂账（无维护窗口数据源）也一并解决。
--
-- 两个标杆的共同口径（刻意保留）：
--   * tracker/announce 不拦——维护期间做种/下载不中断（打断全站 peer
--     的做种，恢复后 H&R、魔力结算全是事故）；
--   * 员工（class_id >= 90）穿透照常全功能访问，维护中还能进后台收尾。
--
-- 三键（ops 分区 / 新「运维」卡——中文 group_key 直接可用，与「注册与邀请」
-- 同做法；不进 settings_group_order，未登记的分区排在尾部不碍事）：
--   maintenance_mode    yes/no，缺省 no。yes = 非 staff 的业务 API 一律
--                       503（错误码 4104），web 侧渲染维护页；
--   maintenance_message 站长自定义维护文案（空 = 前端回落字典默认）；
--   maintenance_until   预计恢复时间（RFC3339；空 = 不显示倒计时——
--                       不造假 ETA 的口径不变，站长填了才显示）。
-- 幂等：ON CONFLICT DO NOTHING，重复执行结果不变。

INSERT INTO site_settings (name, value, descr, grp) VALUES
('maintenance_mode', 'no', '维护模式（yes=全站对非员工显示维护页，员工照常访问）', 'ops'),
('maintenance_message', '', '维护公告文案（空=用默认文案）', 'ops'),
('maintenance_until', '', '预计恢复时间（RFC3339，如 2026-11-06T06:00:00+08:00；空=不显示倒计时）', 'ops')
ON CONFLICT (name) DO NOTHING;

INSERT INTO settings_meta (name, type, label_zh, label_en, hint, options,
                           group_key, card_order) VALUES
('maintenance_mode', 'yesno', '维护模式',
 'Maintenance mode',
 '开启后：非管理组的访问一律显示维护页（API 返回 503/4104），'
 || '管理组（等级 ≥90）穿透照常使用，可进后台随时关闭。'
 || 'tracker/RSS 不受影响——维护期间做种与下载不中断',
 '[{"v":"no","l":"运行中"},{"v":"yes","l":"维护中"}]'::jsonb,
 '运维', 1),
('maintenance_message', 'text', '维护公告文案',
 'Maintenance message',
 '维护页与员工横幅显示的说明文字（支持换行）；留空用默认文案',
 NULL, '运维', 2),
('maintenance_until', 'text', '预计恢复时间',
 'Estimated recovery time',
 'RFC3339 格式（如 2026-11-06T06:00:00+08:00）。填了维护页才显示倒计时，'
 || '留空不显示——不承诺做不到的时间',
 NULL, '运维', 3)
ON CONFLICT (name) DO UPDATE
  SET type = EXCLUDED.type, label_zh = EXCLUDED.label_zh,
      label_en = EXCLUDED.label_en, hint = EXCLUDED.hint,
      options = EXCLUDED.options, group_key = EXCLUDED.group_key,
      card_order = EXCLUDED.card_order;
