-- 0349_resub_uses_user_date_uniq.sql
-- P-93 补签并发双花收口（批三十三实测）：
-- resub_uses 此前只有 idempotency_key UNIQUE，应用层「该日已有签到记录」
-- 的检查在事务外——同用户同一天两张卡并发补签：两个请求都通过检查，
-- SKIP LOCKED 各锁一单、幂等键各不同（resub:{order}），双双落库、一天
-- 耗两卡。唯一约束把「一人一天至多一条补签」钉进库层，应用侧
-- ON CONFLICT (user_id, target_date) DO NOTHING 收口：败者 0 行 → 回滚
-- 不耗卡；娱乐屋扣减挪到闸门之后，并发败者也不再撞 idem 唯一约束抛 500。
CREATE UNIQUE INDEX IF NOT EXISTS resub_uses_user_date_key
    ON resub_uses (user_id, target_date);
