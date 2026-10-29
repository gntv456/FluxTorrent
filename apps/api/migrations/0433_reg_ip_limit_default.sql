-- 0433: 同 IP 注册上限缺省 2→5 + 提示语升级（P-687）。
--
-- 现网实测（2026-10-29 测试站）：开放注册期间两位真实会员（yuanbaohz/
-- lms1990，同一宽带出口）注册后，同小区第三位访客（1863298@qq.com）被
-- 「同一网络环境 24 小时内最多注册 2 个账号」拦在事务外（users/验证/
-- 审计全部零痕迹）。家庭宽带 NAT 共享出口 IP 是常态，缺省 2 对开放
-- 注册轮太紧。代码缺省同步改 5；本迁移只改存量站点现值与提示语。

UPDATE site_settings SET value = '5'
 WHERE name = 'reg_ip_limit' AND value = '2';

UPDATE settings_meta
   SET hint = '计数窗口内同一网络环境最多注册数（0 = 不限制）。IP 只存 sha3-256 摘要，明文不落库。注意家庭/公司宽带是 NAT 共享出口——开放注册期间建议 ≥5，否则同小区/同办公室第三个真实访客会被拦'
 WHERE name = 'reg_ip_limit';
