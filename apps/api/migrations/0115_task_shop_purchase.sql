-- 0115: 任务商店真实兑换端点（五问评审 P1 收口）。
-- task_shop_items 原只有展示字段（name/span/tier/count/cost/stock），
-- 没有任何发放通路——UI 长期显示「即将上线」。本迁移按既有商城发放体系
-- （flux_shared::apply_item_effect）补齐 kind/config、库存已用计数与订单表。

ALTER TABLE task_shop_items ADD COLUMN IF NOT EXISTS kind TEXT;
ALTER TABLE task_shop_items ADD COLUMN IF NOT EXISTS config JSONB NOT NULL DEFAULT '{}'::jsonb;
ALTER TABLE task_shop_items ADD COLUMN IF NOT EXISTS stock_used INT NOT NULL DEFAULT 0;

-- 五档商品映射：只映射有真实发放臂的四种；「射魔一次」旧站无对应实现，
-- kind 留 NULL → 购买口拒单、UI 标「暂未开放」，绝不扣款发空气。
UPDATE task_shop_items SET kind = 'vip', config = '{"days":90}'::jsonb
 WHERE name = 'VIP' AND span = '九十天' AND kind IS NULL;
UPDATE task_shop_items SET kind = 'vip', config = '{"days":30}'::jsonb
 WHERE name = 'VIP' AND span = '三十天' AND kind IS NULL;
UPDATE task_shop_items SET kind = 'invite', config = '{}'::jsonb
 WHERE name = '永久邀请一枚' AND kind IS NULL;
UPDATE task_shop_items SET kind = 'rename_card', config = '{}'::jsonb
 WHERE name = '修改用户名一次' AND kind IS NULL;

-- 兑换订单：idempotency_key 唯一约束是幂等锚点；effect_applied 支持
-- 「扣款成功但发券失败」的补发（同 shop_orders 的 CAS 语义）。
CREATE TABLE IF NOT EXISTS task_shop_orders (
  id BIGSERIAL PRIMARY KEY,
  user_id BIGINT NOT NULL REFERENCES users(id),
  item_id INT NOT NULL REFERENCES task_shop_items(id),
  cost BIGINT NOT NULL,
  idempotency_key TEXT NOT NULL UNIQUE,
  effect_applied BOOLEAN NOT NULL DEFAULT false,
  created_at TIMESTAMPTZ NOT NULL DEFAULT now()
);
CREATE INDEX IF NOT EXISTS idx_task_shop_orders_user
  ON task_shop_orders (user_id, created_at DESC);
