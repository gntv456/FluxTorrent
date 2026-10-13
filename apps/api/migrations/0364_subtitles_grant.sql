-- 0364：新增 subtitles.grant（批六十四 P-236，2026-10-13 字幕链实测）
--
-- 现网事实（闸门 scripts/pt_audit_cd_subtitle_grants.py 的红跑读数）：
--   POST /admin/subtitles/awards/grant  一次调用按名次发出 5000/2000/1000 火花
--                                        并挂 sub_gold/sub_silver/sub_bronze 勋章
--   POST /admin/subtitles/certs/grant   给任意账号盖「certified / gold 字幕组」身份
--   POST /admin/subtitles/certs/revoke  撤销同上
-- 三处过去都只判 staff()。而 staff.panel 挂在 class/90，`authz.rs:96-98` 的
-- class 键是「user_classes.id >= role_key」的阈值语义 ⇒ 91 发布员、92 论坛版主
-- 就能发钱与盖章。这是「钱 + 荣誉」两样一起，比读名单重一档。
--
-- 档位取 class/95（主管及以上），与 0285 给 passkey 明文的定档同口径；
-- 站长若要把评选发奖下放给字幕组组长，改下面这一行的阈值即可（代码不用动）。
-- 幂等；implemented=true，代码侧已接线（CD 的「目录+常量+消费点」三件齐判据
-- 会盯住只加键不接线）。

INSERT INTO permissions (key, name, category, descr, sort, implemented) VALUES
('subtitles.grant', '授予/撤销字幕奖励与认证', 'content',
 '发放金字评选名次奖励（火花+勋章），或授予/撤销 certified/gold 字幕组身份',
 54, true)
ON CONFLICT (key) DO NOTHING;

INSERT INTO role_permissions (role_type, role_key, permission_key) VALUES
('class', '95', 'subtitles.grant')
ON CONFLICT (role_type, role_key, permission_key) DO NOTHING;
