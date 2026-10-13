-- 0365：jixiao.assign 与 reports.handle（批六十五 P-240/243，2026-10-13）
--
-- 现网红证（闸门 scripts/pt_audit_ce_jixiao_money_writes.py 的 CE3/CE4）：
--   POST /admin/users/{id}/jixiao  —— 一个 class 12 的成员、只在用户级覆盖了
--        staff.panel（**低于一切职员档**）就能给另一个成员登记考核，
--        并且 amount 由调用方任意传：探针传 -1 时 **http=200、jixiao_claims
--        0→1 行**。这行 `settled_at IS NULL 即未发放`，jixiao_settle 会据此
--        发钱 ⇒ 负数=凭空倒扣，无上限=手滑可写天量。
--   POST /admin/reports/claim / release / resolve —— 同一个探针能占号、
--        放号、把工单结案（status 0→1）并给举报人发私信。
--
-- 档位（class 键是「id >= role_key」的阈值语义，一行即覆盖其上所有档）：
--   jixiao.assign  class/95 —— 动钱，与 0285 的 user.passkey.reveal、
--                  批六十三的 sparklog.view、批六十四的 subtitles.grant 同档
--   reports.handle class/92 —— 工单流转是版主的日常工作，发布员(91)不该占号
-- 金额界不写死数字：改为「0 ..= 该岗位 base_pay * 3」，由代码里那条
-- jixiao_types 查询一并取出（现网 base_pay 为 5000/8000/6000/20000/50000）。
-- 幂等；implemented=true（代码侧已接线，CE8 的三件齐判据盯住只加键不接线）。

INSERT INTO permissions (key, name, category, descr, sort, implemented) VALUES
('jixiao.assign', '登记成员考核与待发金额', 'user',
 '把成员登记为某考核岗位并写入待发金额（jixiao_claims，未结算前即待发放）',
 76, true),
('reports.handle', '处理举报工单', 'user',
 '认领 / 放号 / 结案或驳回举报（结案会同时给举报人发私信）', 75, true)
ON CONFLICT (key) DO NOTHING;

INSERT INTO role_permissions (role_type, role_key, permission_key) VALUES
('class', '95', 'jixiao.assign'),
('class', '92', 'reports.handle')
ON CONFLICT (role_type, role_key, permission_key) DO NOTHING;
