-- 0392_forum_rules.sql
-- 版块规则条的落库（样图集 § forum-board 的 `.fb-rules` 那一块）
--
-- 现状：`forums` 只有 `descr`（一句话简介，前台版块卡与版块页标题下都
-- 用它）。样图在版块页顶部还有一条「版块规则：1. …；2. …」——本版专属
-- 的规则正文。此前**没有任何列承载它**，版主只能把规则挤进 descr，
-- 结果就是版块卡上出现一大段规则文本（那里只该放一句简介）。
--
-- 本迁移只加一列，不改任何既有行为：
--   * 允许 NULL：NULL = 本版没写规则 ⇒ 前台整条规则条不渲染（不摆空框）；
--   * 不建索引：只在版块页顶部随该版块读，不参与任何检索；
--   * 长度上限写在应用层（admin_http::forum_mods），SQL 侧不做 CHECK，
--     免得以后想放宽规则文案时又要走一次迁移。
ALTER TABLE forums ADD COLUMN IF NOT EXISTS rules text;

COMMENT ON COLUMN forums.rules IS
  '版块规则正文（管理端「论坛结构」维护；前台版块页顶部规则条显示；NULL=未设置）';
