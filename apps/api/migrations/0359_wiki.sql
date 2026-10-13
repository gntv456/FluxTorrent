-- 0359_wiki.sql
-- Wiki 最小落地：独立 wiki_pages / wiki_revisions 两表 + wiki.manage 权限细键
-- + wiki 模块键登记 + 管理台导航条目。
--
-- 形态决策：不与 custom_pages 混表——custom_pages 是「富文本 HTML 单页」
-- （正文经 ammonia 消毒后 dangerouslySetInnerHTML 注水），Wiki 是
-- 「多页 + 版本史 + BBCode 正文」（前端 renderBBCode 渲染 React 节点，
-- 天然免疫 XSS，后端不做 HTML 消毒——这是与 custom_pages 的关键差异）。
-- 版本快照走 rules_revisions（0079）的全量快照口径：不存 diff，编辑时
-- 同事务先 INSERT 旧行进 wiki_revisions 再 UPDATE。
--
-- 幂等：CREATE TABLE IF NOT EXISTS / ON CONFLICT DO NOTHING / WHERE NOT EXISTS。
-- 编号 0359：0354/0355 预留给并行会话（0356/0357 已被占用）。

BEGIN;

-- ============ 1) wiki_pages：页面主表 ============
-- slug 口径与 custom_pages 相同（小写字母/数字/连字符，≤60，应用层校验）。
CREATE TABLE IF NOT EXISTS wiki_pages (
    id BIGSERIAL PRIMARY KEY,
    slug TEXT UNIQUE NOT NULL,
    title TEXT NOT NULL,
    body TEXT NOT NULL,              -- BBCode 正文，200KB 上限在应用层
    visible BOOLEAN NOT NULL DEFAULT true,
    sort INT NOT NULL DEFAULT 0,
    edited_by BIGINT REFERENCES users(id),
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT now()
);

-- ============ 2) wiki_revisions：全量快照（0079 rules_revisions 口径） ============
CREATE TABLE IF NOT EXISTS wiki_revisions (
    id BIGSERIAL PRIMARY KEY,
    page_id BIGINT NOT NULL REFERENCES wiki_pages(id) ON DELETE CASCADE,
    title TEXT NOT NULL,
    body TEXT NOT NULL,
    sort INT NOT NULL,
    edited_by BIGINT REFERENCES users(id),
    created_at TIMESTAMPTZ NOT NULL DEFAULT now()
);

CREATE INDEX IF NOT EXISTS idx_wiki_revisions_page
    ON wiki_revisions(page_id, created_at DESC);

-- ============ 3) 权限细键 wiki.manage（照 0217 custompages.manage 形态） ============
-- 细键缺省未授给任何档，只能站长显式授予；授权继承：凡当前持有
-- custompages.manage 的（同属自建内容域），同授 wiki.manage，granted 原样复制。
INSERT INTO permissions (key, name, category, descr, sort, implemented)
VALUES ('wiki.manage', 'Wiki 管理', 'content',
        'Wiki 页面的增删改与版本历史查看（含目录/导航可达性）', 53, true)
ON CONFLICT (key) DO NOTHING;

INSERT INTO role_permissions (role_type, role_key, permission_key, granted)
SELECT rp.role_type, rp.role_key, 'wiki.manage', rp.granted
FROM role_permissions rp
WHERE rp.permission_key = 'custompages.manage'
ON CONFLICT DO NOTHING;

-- ============ 4) wiki 模块键激活（三处同步：modules 表 / Rust key::ALL / TS MODULE_KEYS） ============
-- 0025/0039 的死开关 enable_wiki('no') 从未接线；模块注册表真值在
-- site_settings.module_<key>（0200 起 modules.is_on 已删）。缺省 no：
-- 站长显式打开才上前台（fail-close，同 contests 先例）。
INSERT INTO modules (key, name_zh, name_en, descr, grp)
VALUES ('wiki', 'Wiki', 'Wiki', '站点 Wiki 多页文档（BBCode 正文 + 版本史）', 'community')
ON CONFLICT (key) DO NOTHING;

INSERT INTO site_settings (name, value, descr, grp)
VALUES ('module_wiki', 'no', 'Wiki（模块开关）', 'module')
ON CONFLICT (name) DO NOTHING;

INSERT INTO settings_meta (name, type, label_zh, label_en, group_key, card_order)
VALUES ('module_wiki', 'yesno', '启用 Wiki', 'Wiki', 'module_community', 99)
ON CONFLICT (name) DO NOTHING;

-- ============ 5) 管理台导航条目（tab_key 与前端 renderSimpleTool case 一致） ============
-- module_key 用 NULL（空串是迁移手写列踩哨兵的产物，admin_nav_align_guard 判红）
INSERT INTO staff_panel_entries
    (panel, name, url, info, sort, section, min_class, tab_key,
     perm_key)
SELECT 'admin', 'Wiki 管理', '/admin?tool=wiki',
       '编辑/增加/删除 Wiki 页面（含版本历史）',
       12, 'content', 75, 'wiki', 'wiki.manage'
WHERE NOT EXISTS (
    SELECT 1 FROM staff_panel_entries WHERE tab_key = 'wiki'
);

COMMIT;
