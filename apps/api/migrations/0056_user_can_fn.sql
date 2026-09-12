-- 权限判定下沉为 SQL 函数：API 与 worker 共用同一实现，避免两处逻辑漂移
--
-- 背景：hr.exempt（免除 H&R）需要在 worker 的批量 SQL 里判定，若在 worker 里
--       重写一份判定逻辑，日后改规则必然出现「API 已经改了、worker 没跟上」的漂移。
-- 方案：把 authz::user_can 的判定规则固化为 STABLE 函数，Rust 侧改为调用它。
--
-- 判定优先级：用户级覆盖 > 角色（等级累进 ∪ 职务）

CREATE OR REPLACE FUNCTION user_can(p_user_id bigint, p_perm text)
RETURNS boolean
LANGUAGE sql
STABLE
AS $$
    SELECT COALESCE(
        (SELECT up.granted FROM user_permissions up
         WHERE up.user_id = p_user_id AND up.permission_key = p_perm),
        EXISTS (
            SELECT 1 FROM role_permissions rp
            JOIN users u ON u.id = p_user_id
            WHERE rp.permission_key = p_perm
              AND rp.granted
              AND (
                (rp.role_type = 'class'
                 AND CASE WHEN rp.role_key ~ '^[0-9]+$'
                          THEN u.class_id >= rp.role_key::integer
                          ELSE FALSE END)
                OR
                (rp.role_type = 'role'
                 AND EXISTS (
                     SELECT 1 FROM user_roles ur
                     WHERE ur.user_id = p_user_id
                       AND ur.role_key = rp.role_key
                       AND (ur.expires_at IS NULL OR ur.expires_at > now())
                 ))
              )
        )
    );
$$;

COMMENT ON FUNCTION user_can(bigint, text) IS
    '权限判定：用户级覆盖 > 角色（等级累进 ∪ 职务）。与 authz::user_can 同源。';
