-- 0516: 猜大小由即时局改为倒计时房间制（0515），大厅分组从 instant 移到 session。
-- 仅改展示归类，不动经济参数；幂等 UPDATE，旧库补跑也安全。

UPDATE arcade_games SET grp = 'session', updated_at = now()
 WHERE key = 'bigsmall' AND grp = 'instant';
