-- 0316 用户级列表偏好（2026-10-08 二轮实证缺口）
--
-- 现状：列表页的多视图 / 每页条数 / 列显隐**已有**，但都是**站点级**
-- （site_settings.view_hidden，E6 视图布局，全站统一）。
-- 缺口：用户不能按自己习惯覆盖——对标 Arcadia layout config 的用户级偏好。
--
-- 建模：复用 users.notice_prefs 的 JSONB 同范式（无独立偏好表）。
-- 键白名单由 API 端点把关（GET/POST /me/list-prefs）：
--   view        TEXT   'table' | 'card' | 'poster'
--   per_page    INT    20 / 50 / 100
--   hidden_cols TEXT[] 用户额外隐藏的列（叠加在站点级 view_hidden 之上）
--
-- 缺省 '{}' = 完全走站点默认，行为不变（向后兼容，零回归）。

ALTER TABLE users
  ADD COLUMN IF NOT EXISTS list_prefs JSONB NOT NULL DEFAULT '{}'::jsonb;
