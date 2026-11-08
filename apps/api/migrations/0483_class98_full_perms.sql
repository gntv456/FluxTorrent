-- 0483: 维护开发员（class 98）升级为「全权」——实时继承 99 的 class 授权
--（号让位：0482 批次已被变更日志批占用，本件顺延 0483）
--
-- 现象：站长在权限矩阵把发布（torrent.upload）等 21 把键授给 99 档后，
--   98 档三个号 user_can('torrent.upload') 全为 false。0472 的「接近站长」
--   是执行时快照镜像：站长日后在矩阵里给 99 新授的键不会跟到 98，每次
--   改矩阵都要把 98 列再点一遍，漏了就出「维护开发员没有发布权限」。
-- 决定（站长定调：维护开发员应该有所有权限）：0472 的「接近站长」收口
--   为「全权」，两段落地：
--   1) user_can() 的 class 分支加继承条款：class_id >= 98 时，凡授给
--      class/99 的键一律视为持有——授权跟着矩阵编辑实时走，快照漂移
--      从根上消灭。用户级覆盖仍最优先（显式拒绝照样压过继承），
--      职务逻辑不动。
--   2) 一次性把 99 现有授权动态补镜像到 98（0472 §1 同款动态复制），
--      让权限矩阵 UI 的 98 列与实际生效一致，免得界面假空再引误判。
--      注意：此后 98 列的「取消勾选」对 99 继承来的键不生效（生效面以
--      99 列为准）；要单独收回某个 98 账号的某把键，走用户级显式拒绝。
-- 保留的护城河（不动，仍只有 99）：不能把任何人设为 99
--   （user_status.rs:244）、不能改 99 账号（ensure_outranks）、批量改档
--   值域封顶 98（users_batch.rs:42）。98 能用全站所有功能，但造不出、
--   也动不了站长本人。
-- 幂等：CREATE OR REPLACE 可重放；镜像 ON CONFLICT DO UPDATE；
--   防呆只读 + 探针 NOTICE，重放无副作用。

BEGIN;

-- ============ 1) user_can()：98 档继承 99 的 class 授权 ============
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
                               OR (u.class_id >= 98
                                   AND rp.role_key::integer = 99)
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
    '权限判定：用户级覆盖 > 角色（等级累进 ∪ 职务；class>=98 实时继承 class/99 全部授权）。与 authz::user_can 同源。';

-- ============ 2) 99 现有授权补镜像到 98（UI 对账用） ============
INSERT INTO role_permissions (role_type, role_key, permission_key, granted)
SELECT 'class', '98', rp.permission_key, true
  FROM role_permissions rp
 WHERE rp.role_type = 'class' AND rp.role_key = '99' AND rp.granted
ON CONFLICT (role_type, role_key, permission_key)
DO UPDATE SET granted = true;

-- ============ 3) 防呆 ============
DO $$
DECLARE
    n99 int;
    n98 int;
    n_bad int;
BEGIN
    -- 镜像对账：99 凭 class 持有的键，98 行上必须一把不少
    -- （继承条款在函数层兜底，这里对的是可见行数）
    SELECT count(*) INTO n99 FROM role_permissions
     WHERE role_type = 'class' AND role_key = '99' AND granted;
    SELECT count(*) INTO n98 FROM role_permissions
     WHERE role_type = 'class' AND role_key = '98' AND granted;
    IF n98 < n99 THEN
        RAISE EXCEPTION '98 镜像不全：99 实有 % 把，98 只落 % 把', n99, n98;
    END IF;

    -- 探针（只告警不阻断）：98+ 的在册账号，除显式拒绝者外，
    -- 必须能过 user_can 的发种键
    SELECT count(*) INTO n_bad
      FROM users u
     WHERE u.class_id >= 98
       AND u.status < 2
       AND NOT EXISTS (
            SELECT 1 FROM user_permissions up
             WHERE up.user_id = u.id
               AND up.permission_key = 'torrent.upload'
               AND up.granted = false)
       AND NOT user_can(u.id, 'torrent.upload');
    IF n_bad > 0 THEN
        RAISE NOTICE '% 个 98+ 账号 torrent.upload 仍为 false，请人工核对', n_bad;
    END IF;
END $$;

COMMIT;
