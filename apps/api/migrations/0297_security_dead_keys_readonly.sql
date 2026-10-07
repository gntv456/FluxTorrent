-- 0297（四审 S1）：安全死键簇收编 readonly。
-- 背景：maxip / maxloginattempts / loginattemptwhitelist / securelogin /
-- securetracker 五键自 0034 从 NP 对齐种入，但**全仓零代码消费方**——
-- 登录限流实际由 auth_infra::throttle（5/min 固定）与 acctlock 锁定承担，
-- tracker 走 https 与否由部署层（域名/反代）决定。
-- 留在设置中心可编辑的害处：站长把「失败登录锁定阈值」从 5 改成 20，
-- 以为放宽了爆破防线，实际什么都没发生——安全面板变成安慰剂，
-- 真正的防线参数反而没人知道改不动。
-- 处置：settings_meta 置 readonly（面板灰显 + 导入导出跳过），站点值保留
-- 不动（兼容旧导出包的 diff），hint 指明真防线所在。不删行：删了会把
-- 0034 的种子行炸成悬空（site_settings 行仍在），readonly 是最小可逆处置。
UPDATE settings_meta SET readonly = true, hint =
  '兼容保留键：本站登录防线由内置限流（用户名+IP 双维 5/分）与账号锁定承担，'
  '此项不再参与实际逻辑。'
WHERE name IN ('maxloginattempts', 'loginattemptwhitelist', 'maxip');
UPDATE settings_meta SET readonly = true, hint =
  '兼容保留键：Tracker 是否 HTTPS 由部署层（域名与反代）决定，此项不再参与实际逻辑。'
WHERE name IN ('securelogin', 'securetracker');
