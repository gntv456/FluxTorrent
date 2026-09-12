-- 用户级权限覆盖：在「角色权限」之上支持对单个用户逐项授予/拒绝具体权限
--
-- 背景：role_permissions 只能按角色（等级 / 职务）批量配权，无法满足
--       「临时给某人开一个权限」「某人虽有角色但单独禁掉某项」这类运营需求。
--       对标 NexusPHP 角色插件的 user_permissions。
--
-- 判定优先级（见 authz::user_can）：
--   用户级覆盖（本表有记录）> 角色权限（等级累进 ∪ 职务）
--   granted=true  → 额外授予（即使角色没有）
--   granted=false → 显式拒绝（即使角色有）
--   无记录        → 继承角色判定

CREATE TABLE IF NOT EXISTS user_permissions (
    user_id        bigint NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    permission_key text   NOT NULL REFERENCES permissions(key) ON DELETE CASCADE,
    granted        boolean NOT NULL,
    granted_by     bigint,
    granted_at     timestamptz NOT NULL DEFAULT now(),
    note           text,
    PRIMARY KEY (user_id, permission_key)
);

CREATE INDEX IF NOT EXISTS idx_user_perms_user ON user_permissions (user_id);

-- 导航登记：权限配置页签
INSERT INTO staff_panel_entries (panel, name, url, info, sort, section, min_class, tab_key) VALUES
('sysop', '权限配置', '/admin?tool=perm', '角色权限矩阵与用户级权限分配', 12, 'system', 99, 'perm')
ON CONFLICT DO NOTHING;
