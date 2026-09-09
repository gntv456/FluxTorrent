-- 0017: 空库引导 —— 站长（root）账户与最小种子。
-- 之前 root 是开发库手工数据，compose 全新卷启动时无任何用户，站点无法登录管理。
-- Argon2id 哈希对应密码 password123（与开发环境一致，生产首登必须改密）。
INSERT INTO users (id, username, email, pass_hash, passkey, class_id, status, spark_balance)
VALUES (
    1, 'root', 'root@fluxtorrent.local',
    '$argon2id$v=19$m=19456,t=2,p=1$XAMwi8WTuzejCBPhdilR6w$xUb/nkW8/iUYTMb+dCPsstkyeldF5LuM2sOIK4m8++c',
    'rootbootstrap0000000passkey00', 99, 0, 100000
) ON CONFLICT DO NOTHING;

-- 必须重置序列，否则后续注册的用户会与 id=1 冲突
SELECT setval('users_id_seq', GREATEST((SELECT max(id) FROM users), 1));
