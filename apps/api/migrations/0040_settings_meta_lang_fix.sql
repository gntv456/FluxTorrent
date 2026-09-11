-- 0040_settings_meta_lang_fix.sql
-- 修正 0039 中 default_language / defaultlang 的枚举口径。
--
-- 背景：0039 误用了 Web 路由的 locale 码（zh-CN / zh-TW / en），但本站「语言」这一维度
-- 的既有约定是 NexusPHP 风格的三码 en|chs|cht：
--   * 0024_usercp_settings.sql: users.site_language DEFAULT 'chs'
--   * 0025_staffpanel.sql:      site_settings('default_language','chs')
--   * apps/web/components/usercp.tsx: <option value="chs|cht|en">
-- 结果是 default_language 的既有值 'chs' 不在枚举选项内 —— 前端下拉会静默落到首个选项，
-- 一旦保存就篡改配置。本迁移把两个键的选项对齐到 en|chs|cht。
--
-- 之所以追加为 0040 而非直接改 0039：0039 已应用到开发库，就地修改会触发
-- sqlx「migration 39 was previously applied but has been modified」的校验和不一致错误。
--
-- 幂等：纯 UPDATE，可重复执行。

UPDATE settings_meta
SET options = '[{"v":"en","l":"English"},{"v":"chs","l":"简体中文"},{"v":"cht","l":"繁體中文"}]'::jsonb
WHERE name IN ('default_language', 'defaultlang')
  AND type = 'enum';

-- defaultlang 由 0039 补种时填的是 'zh-CN'，对齐到项目口径 'chs'
UPDATE site_settings SET value = 'chs'
WHERE name = 'defaultlang' AND value = 'zh-CN';

-- 说明该键与 default_language 的关系，避免管理员困惑于两个「默认语言」
UPDATE settings_meta
SET hint = 'NexusPHP 兼容键，需与「默认语言」保持一致；仅旧版脚本读取时生效'
WHERE name = 'defaultlang';
