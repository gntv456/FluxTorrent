-- 批六十（P-230）：torrent.set_price 标 implemented=false。
--
-- 0062 那批「修正 implemented 标记」漏了它：apps/api/src/authz/perm.rs 里这个
-- 常量自带 #[allow(dead_code)] 与注释「付费种子改价端点未实装（价格仅发布时
-- 设置），接线时删除本 allow」，可目录里仍是 implemented=true。后果不是「少个
-- 徽章」：apps/web 的 staff-tools-perm.tsx:211 与 staff-tools-perm-matrix.tsx:87
-- 只在 implemented=false 时挂「未实装」提示，于是权限矩阵把这项能力当可用展示，
-- 站长给某角色勾上「设置促销价格」后不会有任何生效，也不会有任何提示。
--
-- 常驻门禁：scripts/pt_audit_bz_perm_truth.py
--   Z2  矩阵里 implemented=true 却没有代码消费点的键（消费点两种形态都认：
--       Rust 的 perm::NAME 引用，以及 SQL 里的 user_can(…, 'key')——
--       hr.exempt 只有后者，别把它误报成零消费）
--   Z3  反向漂移：implemented=false 却已有消费点（能力被埋）
-- 接线「付费种子改价」端点时，把本行改回 true 并删掉 perm.rs 的 allow。

UPDATE permissions SET implemented = false
 WHERE key = 'torrent.set_price' AND implemented = true;
