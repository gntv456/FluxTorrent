-- 0293 法币捐赠档位 reward 文案补齐（商城审计 P2-5）
-- donation_plans.reward 是纯展示列（donate 页「赠送」栏），1-3 档为 NULL
-- 渲染成空白。补「无额外赠送」口径，与 4-6 档「获赠邀请 1」对齐。
UPDATE donation_plans SET reward = '无额外赠送'
WHERE reward IS NULL OR reward = '';
