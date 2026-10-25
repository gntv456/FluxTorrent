-- 0419: 补种 0171/0286 的两个站点设定键（2026-10-25 发布组深测实测立案）。
--
-- 现象：本站 site_settings/settings_meta 里 upload_auto_approve_class 与
-- upload_title_pattern 双双缺失（10-18 备份即无；0171/0286 均按
-- ON CONFLICT DO NOTHING 幂等插入且迁移台账 success）。代码读取方
-- （manage_perm.rs / upload_precheck.rs）对缺行静默回落内置默认
-- （92 / 空），于是：
--   * 站长在设置页永远看不到「发布免审最低等级」卡片（META_SELECT 是
--     site_settings JOIN settings_meta 内连接，行没了卡片就不渲染），
--     也永远改不了免审线；
--   * 「发种命名规范」同理从设置页消失。
-- 成因无法从代码路径复现（全仓仅两处 DELETE FROM site_settings，均带
-- THEME_KEYS/RULE_ 白名单；审计台账 374 条 setting:update 无此键），
-- 判定为建库初期的库外操作所致。存量站是否丢行不可知，故本迁移幂等补种：
--   * 行缺失 → 按出厂默认重建（尊重 0171/0286 原文）；
--   * 行存在 → 一字不动（站长改过 91 就保持 91）。
-- 两个键都不带业务语义变更：默认值与代码内置回落值完全一致。

INSERT INTO site_settings (name, value, descr, grp) VALUES
('upload_auto_approve_class', '92',
 '发布免审最低等级（≥该等级发布即通过；92=论坛版主）', 'torrent'),
('upload_title_pattern', '',
 '发种名称需匹配的 Java 正则（命名规范），留空即不校验。'
 '例：^[^.]+\.[0-9]{4}\.(1080p|2160p|720p) 只放行「片名.年份.分辨率」格式。',
 'torrent')
ON CONFLICT (name) DO NOTHING;

INSERT INTO settings_meta (name, type, label_zh, label_en, hint, group_key, card_order) VALUES
('upload_auto_approve_class', 'classlevel', '发布免审最低等级',
 'Auto-approve min class',
 '等级 ≥ 该值的用户发布种子免审直接通过；该等级作者编辑自己的种子也不再回退待审',
 'torrent', 72),
('upload_title_pattern', 'text', '发种命名规范（正则）',
 'Title pattern (regex)',
 '留空不校验；填 Java 正则，不匹配即拒。改坏正则会在发种时报「站点命名正则非法」。',
 'torrent', 84)
ON CONFLICT (name) DO NOTHING;

-- 同批补种：torrent_edit_class / torrent_delete_class 被 manage_perm.rs
-- 读取（编辑/删除他人种子的等级阈值，缺省 94），但 0171 只在注释里提到
-- 却从未 INSERT——站长设置页没有这两张卡，阈值永远锁死在代码默认值。
-- 默认值与代码回落值一致，不改变现有行为，只是让它们可配置。
INSERT INTO site_settings (name, value, descr, grp) VALUES
('torrent_edit_class', '94', '编辑他人种子所需最低等级（94=管理员）', 'torrent'),
('torrent_delete_class', '94', '删除他人种子所需最低等级（94=管理员）', 'torrent')
ON CONFLICT (name) DO NOTHING;

INSERT INTO settings_meta (name, type, label_zh, label_en, hint, group_key, card_order) VALUES
('torrent_edit_class', 'classlevel', '编辑他人种子等级',
 'Edit-others min class', '等级 ≥ 该值的用户可编辑任何人的种子；作者本人始终可编辑自己的种子', 'torrent', 73),
('torrent_delete_class', 'classlevel', '删除他人种子等级',
 'Delete-others min class', '等级 ≥ 该值的用户可删除任何人的种子；作者仅可删除自己未过审的种子', 'torrent', 74)
ON CONFLICT (name) DO NOTHING;

