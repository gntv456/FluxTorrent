-- 0272：深测四轮（2026-10-03）两件修复的落库部分
--
-- 1) initial_class 站点设定键：注册初始等级（repo/auth.rs register_user 消费）。
--    背景：此前注册 INSERT 不写 class_id → DB 缺省 0（种子级零授权），新用户
--    发种 403，需等 worker class_auto_adjust（最长 5 分钟）才提到 1（新芽）。
--    缺省 1 与权限种子（torrent.upload 授 class≥1）对齐；站长可在后台改。
INSERT INTO site_settings (name, value)
VALUES ('initial_class', '1')
ON CONFLICT (name) DO NOTHING;

-- 列名勘误（2026-10-03）：settings_meta 实际列是 group_key / card_order，
-- 原写成 grp / sort 导致本迁移执行失败 → api 崩溃循环、全站 API 下线。
INSERT INTO settings_meta
  (name, type, label_zh, label_en, hint, min, max, step, group_key, card_order)
VALUES ('initial_class', 'number', '注册初始等级', 'Initial class on signup',
        '新注册用户的起始用户组 id（0-89；1=新芽即具备发种等基础权限）', 0, 89, 1, 'user', 45)
ON CONFLICT (name) DO NOTHING;

-- 2) 修复 0266 orphan_offset 对冲行对 root 余额的污染：
--    0266 把 -52374 对冲行挂在 user 1 名下（当时注释明确「只作全站合计对账，
--    不改个人余额」），但 seeding.rs 的个人余额重算公式 sum(ledger) 不区分——
--    首个重算周期后 root 快照变 365-52374=-52009，此后游戏/签到在负数上滚动
--    （深测实测 -51619）。代码侧已把重算公式排除 kind='orphan_offset'；
--    存量污染无法「删行」（账本只追加纪律），这里按账本纪律补一条反向修正行，
--    使 root 的 sum(流水) 回到真实可用额。
DO $$
DECLARE
    polluted bigint;
    offset_amt bigint;
BEGIN
    SELECT COALESCE(sum(amount), 0) INTO offset_amt
    FROM spark_ledger WHERE user_id = 1 AND kind = 'orphan_offset';
    IF offset_amt = 0 THEN RETURN; END IF;
    -- root 当前快照已被手工纠偏为 0（深测过程中 UPDATE）；补一条正向修正流水
    -- 让「快照 == sum(流水)」重新成立（重算公式排除对冲行后以本行为准）。
    INSERT INTO spark_ledger (id, user_id, amount, kind, ref_type, idempotency_key, balance_after)
    SELECT nextval('spark_ledger_id_seq'), 1, -offset_amt, 'admin', 'fix',
           'fix-orphan-offset-root-20261003', 0
    WHERE NOT EXISTS (
        SELECT 1 FROM spark_ledger WHERE idempotency_key = 'fix-orphan-offset-root-20261003'
    );
END $$;
