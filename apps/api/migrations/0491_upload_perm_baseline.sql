-- 0491：发布权限基线回补——torrent.upload 授 class ≥1 全员
--
-- 现象（好学站 2026-11-16 站长报「是不是没开放发布」）：
--   普通用户（class 1+）全都没有发布权限，矩阵里只剩
--   class 98 / 99 / role uploader 三行。
--
-- 根因：0054_permissions.sql 基线层显式授 class '1'（"基础层：所有
--   注册用户"），role_permissions 矩阵语义是「role_key 为数字时按
--   class_id >= role_key 继承」（authz.rs user_perm_keys）——但 0472
--   权限矩阵重整时把这行收掉了，只留 98/99 与 uploader 职务。与
--   0483（98 档快照镜像漂移）同族：矩阵收权把基线层一并收走。
--
-- 修法：补回 class 1 行（>=1 即全员继承，含未来新注册）。不授
--   torrent.upload_special（受限类别）与 torrent.approval.auto
--   （免审）——那两把仍在 98+/uploader，发布走人工审。
INSERT INTO role_permissions (role_type, role_key, permission_key, granted)
SELECT 'class', '1', 'torrent.upload', true
WHERE NOT EXISTS (
  SELECT 1 FROM role_permissions
  WHERE role_type = 'class' AND role_key = '1'
    AND permission_key = 'torrent.upload'
);
