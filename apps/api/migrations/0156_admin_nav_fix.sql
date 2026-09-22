-- 0156 管理端导航修正（配合「去掉双套导航」）：
--
--  背景：`/admin` 页左侧导航完全由本表驱动，但面板内部还并存一行 35 个
--  药丸按钮，同一功能有两个入口同屏。药丸行删除后，导航成为唯一入口，
--  本表必须先把下面三处补齐/纠正，否则会出现「点了空白」或「点不动」。
--
--   1) 保种统计（seedstats）只存在于药丸行，本表无对应条目 —— 删除药丸后
--      彻底不可达。补入 system 组，与「统计」相邻。
--   2) 邮件群发 url 写成 `?tool=massmail`，而 tab_key 是 `mail`。
--      `handleTool` 用 tab_key 设值、url 只做「是否外链」判断，故点击正常；
--      但该 url 一旦被书签/深链（`/admin?tool=massmail`），LEGACY_TOOL 里并无
--      `massmail` 键 → 落进 panelEmpty 空面板。改为与 tab_key 一致，
--      并在前端 LEGACY_TOOL 补 `massmail -> mail` 兼容老书签。
--   3) 绩效考核 url 写成 `#jixiao`（0106 插入时即错）。它不以 `/admin?tool=`
--      开头，`handleTool` 会走「外链」分支执行 `location.href = "#jixiao"`,
--      只改哈希、永不切工具 —— 该管理面板从未被点开过。0106 的注释已写明
--      目标是 admin-jixiao 面板，故改回 `?tool=jixiao`。
--   4) 删除「促销公告」(promo) 条目：其面板 OpsPromoTab 早已不被渲染（同打
--      `/admin/freeleech`，职能已由「免费/促销状态」承接），点击只会得到
--      空面板。前端已一并移除 ToolTab / 死代码，老书签由 LEGACY_TOOL 兜底。

-- ---- 1) system 组重排序：给 seedstats 留出「统计」之后的位置 ----
UPDATE staff_panel_entries SET sort = sort + 1
WHERE section = 'system' AND sort >= 4;

INSERT INTO staff_panel_entries
  (panel, name, url, info, sort, section, min_class, tab_key)
VALUES
  ('moderator', '保种统计', '/admin?tool=seedstats',
   '保种时长分布 / 完成率 / 各等级保种排行', 4, 'system', 90, 'seedstats')
ON CONFLICT DO NOTHING;

-- ---- 2) 邮件群发：url 与 tab_key 对齐 ----
UPDATE staff_panel_entries
SET url = '/admin?tool=mail'
WHERE tab_key = 'mail' AND url = '/admin?tool=massmail';

-- ---- 3) 绩效考核：`#jixiao` 改回可路由的内链 ----
UPDATE staff_panel_entries
SET url = '/admin?tool=jixiao'
WHERE tab_key = 'jixiao' AND url = '#jixiao';

-- ---- 4) 促销公告：陈旧重复入口，删除（职能已在「免费/促销状态」与「运营配置」）----
DELETE FROM staff_panel_entries WHERE tab_key = 'promo';
