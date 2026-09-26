-- 0216：G1 收尾 —— SITENAME 降级内部镜像键 + defaultlang 退役 + default_language 真驱动
--
-- 背景（承 0214 站名收敛 / 0215 死键退役）：
--   * 站名权威键 = site_name（向导与后台主写它），保存时单向镜像到 SITENAME
--     （settings_http/groups.rs）；site_title / site_subtitle 已在 0215 退役。
--   * SITENAME 仍被 RSS 频道名（rss_http.rs / forum.rs）与站点档案品牌回落读，
--     数据行必须保留；但它已是派生物，不应再出现在设置表单里与 site_name 分叉。
--   * defaultlang 是 0039 补种的 NP 兼容键，全仓零读取（真源是 default_language），
--     0040 曾对齐其枚举，本轮直接退役。
--   * default_language 此前只登记不消费（假开关）：本轮起真实驱动前台默认语言
--     （web i18n/server.ts getLocale 对无 cookie 访客的回落，见同批代码改动）。
--
-- 幂等：UPDATE/DELETE 可重复执行。

-- 1) SITENAME：隐藏（不参与表单渲染）+ 只读（经 groups 保存会被拒），
--    数据行保留 —— RSS/品牌回落仍在读，镜像由 site_name 保存路径维护。
--    与 settings_group_order / cache_driver 同属「程序读、人不可改」的内部键口径。
UPDATE settings_meta
SET visible = false,
    readonly = true,
    hint = '由「站点名称」自动同步（RSS 频道名/邮件署名读取），请勿直接编辑'
WHERE name = 'SITENAME';

-- 2) site_name：升格为「站点名称」（原为「站点简称」，与 SITENAME 的旧标签倒置）
UPDATE settings_meta
SET label_zh = '站点名称',
    label_en = 'Site name',
    hint = '全站标题、邮件署名与 RSS 频道名；保存后自动同步内部键 SITENAME'
WHERE name = 'site_name';

-- 3) defaultlang：NP 兼容死键退役（meta 行随 site_settings 行外键级联删除）
DELETE FROM site_settings WHERE name = 'defaultlang';

-- 4) default_language：补 hint，言明消费点（未选语言的新访客）
UPDATE settings_meta
SET hint = '未选择语言的新访客默认看到的语言；登录用户以个人设置为准'
WHERE name = 'default_language';
