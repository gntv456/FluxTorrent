-- BUG 修复：admin_p3_http.rs 的 /admin/user-medals 查询引用 um.granted_at，
-- 但 user_medals 表（0001_init 建表）一直没有该列，接口 100% 500（内部错误）。
-- 授予时间语义与 user_roles.granted_at（0054）对齐：存量行回填授予时间不可考，取 now()。

ALTER TABLE user_medals
  ADD COLUMN IF NOT EXISTS granted_at TIMESTAMPTZ NOT NULL DEFAULT now();

COMMENT ON COLUMN user_medals.granted_at IS '授予时间（存量行为迁移时刻）';
