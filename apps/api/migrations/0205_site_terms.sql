-- 0205：术语表（四审 L7「无术语机制」——站长无法把「种子」统一改叫「资源」）。
--
-- 为什么是**表**而不是 site_settings 的一个键：术语是「一组可增删的行」，
-- 塞进 KV 就得在值里编分隔符（`forum_banned_words` 那种换行分隔 text 就是前车之
-- 鉴：不可索引、不可逐条启停、长度截断）。这是四审根因 R2「该是行的东西写成了
-- 列」在文案层的同一种偏向。
--
-- 生效口径（三层同一份规则，见 apps/api/src/terms.rs / apps/web/i18n/apply-terms.ts）：
--   canonical（文案里写死的规范词）→ replacement（本站叫法），单次正向扫描替换，
--   替换结果不再参与匹配（防级联），最长词优先。
--
-- **零预置行**：新装站上没有规则，已有站点升级后文案一字不变。
-- 预置规则由站型包携带（0206 的 site_type_packs.terms），教育包可以自带
-- 「种子→学习资源」，站长也可以自己加/停。

CREATE TABLE IF NOT EXISTS site_terms (
    canonical   TEXT PRIMARY KEY,
    replacement TEXT NOT NULL,
    enabled     BOOLEAN NOT NULL DEFAULT true,
    descr       TEXT NOT NULL DEFAULT '',
    sort        INTEGER NOT NULL DEFAULT 100,
    updated_at  TIMESTAMPTZ NOT NULL DEFAULT now(),
    CHECK (length(btrim(canonical)) > 0),
    CHECK (length(btrim(replacement)) > 0),
    -- 自我替换是空转，登记进来只会让人以为改了什么
    CHECK (canonical <> replacement)
);

-- 后台方向盘（tab_key 与 staff-tools 的 ToolTab、STAFF_TOOL_TABS 三方一致）
INSERT INTO staff_panel_entries
    (panel, name, url, info, sort, section, min_class, tab_key)
SELECT 'admin', '术语表', '/admin?tool=terms',
       '把「种子 / 魔力 / 保种」这类固有词改成本站的叫法（全站文案与提示同改）',
       11, 'content', 93, 'terms'
WHERE NOT EXISTS (
    SELECT 1 FROM staff_panel_entries WHERE tab_key = 'terms'
);
