-- 0363：三条敏感职员读口的专属档位（批六十三 P-233~235）
--
-- 现网事实（闸门 scripts/pt_audit_cb_sensitive_views.py 的红跑读数）：
--   探针账号只有 staff.panel 就能读到
--     GET /admin/users         → 20 行会员名单（库内 status<2 共 98 人）
--     GET /admin/login-logs    → 20 行「用户名 + 来源 IP + 成败」（库内 3743 行）
--     GET /admin/spark-logs    → 20 行魔力流水，含 balance_after 余额（库内 2097 行）
--     GET /admin/torrent-buys  → 10 行种子/代币购买流水
--     GET /admin/users/{id} 与 {id}/snatches → 200（邮箱、做种时长、HR 标记）
--   根因与 P-231 同一处：`authz.rs:96-98` 对 role_type='class' 是
--   `user_classes.id >= role_key` 的**阈值**语义，而 staff.panel 挂在
--   class/90 ⇒ 91 发布员、92 论坛版主、93 总版主、94 管理员全都有职员门。
--
-- 分档依据（按数据敏感度，非按「谁是同事」）：
--   user.view      class/92 —— 版主处理举报/申述要能查人；发布员(91)不需要名册
--   loginlogs.view class/93 —— 登录轨迹与来源 IP 属安全事件，总版主起
--   sparklog.view  class/95 —— 余额与资金流水，与 0285 的 user.passkey.reveal 同档
-- class 是阈值语义，故三行分别覆盖 92..99 / 93..99 / 95..99。
-- 全部幂等；三键 implemented=true（代码侧已接线，CB5 的「目录+常量+消费点」
-- 三件齐判据会盯住只加键不接线的情况）。

INSERT INTO permissions (key, name, category, descr, sort, implemented) VALUES
('user.view', '查看会员名单与详情', 'user',
 '读取会员名册、详情与下载/做种流水；不含 passkey 明文', 73, true),
('loginlogs.view', '查看登录日志', 'system',
 '读取登录事件（用户名、来源 IP、成败），用于安全事件回溯', 113, true),
('sparklog.view', '查看魔力与购买流水', 'system',
 '读取 spark_ledger 流水（含 balance_after 余额）与种子/代币购买记录', 114, true)
ON CONFLICT (key) DO NOTHING;

INSERT INTO role_permissions (role_type, role_key, permission_key) VALUES
('class', '92', 'user.view'),
('class', '93', 'loginlogs.view'),
('class', '95', 'sparklog.view')
ON CONFLICT (role_type, role_key, permission_key) DO NOTHING;
