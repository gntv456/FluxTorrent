-- 0187：自定义页面（一审 R4.4——FAQ/规则之外站长可造任意内容页）。
--
-- custom_pages：slug/标题/富文本/可见性/排序；菜单系统（menu_items）已有
-- 树形 CRUD，站长把自定义菜单项指向 /p/{slug} 即完成导航挂接。
-- body 为管理员撰写的 HTML（NP 口径）——展示侧 ammonia 白名单消毒后输出
--（与公告同款防线：管理员账号被盗也不构成全站存储 XSS）。

CREATE TABLE IF NOT EXISTS custom_pages (
    id          BIGSERIAL PRIMARY KEY,
    slug        TEXT NOT NULL UNIQUE,       -- 小写字母/数字/连字符 ≤60
    title       TEXT NOT NULL,
    body        TEXT NOT NULL DEFAULT '',   -- 富文本 HTML（展示侧消毒）
    visible     BOOLEAN NOT NULL DEFAULT TRUE,
    sort        INT NOT NULL DEFAULT 100,
    created_at  TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at  TIMESTAMPTZ NOT NULL DEFAULT now()
);

CREATE INDEX IF NOT EXISTS idx_custom_pages_sort ON custom_pages (sort, id);
