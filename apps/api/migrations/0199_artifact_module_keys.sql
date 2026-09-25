-- 0199：让站长自建产物也能挂模块开关（四审 L5「反向能力」延伸）
--
-- 29→30 个模块的开关在前台四处一致，`menu_items` 也从 0107 起有 module_key；
-- 但站长自建的两种产物——自定义页面（0187）与用户自定义字段（0186）——没有这个挂点：
-- 站长关掉 subtitles 之后，为字幕组写的自定义页面仍能从 /p/{slug} 打开、
-- 注册页仍在要「字幕组工号」这类字段。本迁移给两张表补上可空 module_key。
--
-- 语义沿用 menus_public.rs 的既有拍板（T3）：module_key 非空且对应
-- site_settings.module_<key> 不为 'yes' ⇒ 视为关闭（未配置键按关处理）。
ALTER TABLE custom_pages
    ADD COLUMN IF NOT EXISTS module_key text;

ALTER TABLE user_field_defs
    ADD COLUMN IF NOT EXISTS module_key text;

-- 键合法性在写入侧校验（后台 CRUD 会拒未知键）；这里只补索引，
-- 公开读路径（/p/{slug}、注册字段、档案、usercp）都按模块过滤。
CREATE INDEX IF NOT EXISTS idx_custom_pages_module
    ON custom_pages (module_key) WHERE module_key IS NOT NULL;
CREATE INDEX IF NOT EXISTS idx_user_field_defs_module
    ON user_field_defs (module_key) WHERE module_key IS NOT NULL;
