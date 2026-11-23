-- 0518: 「APP 专属 VIP(30天)」下架。
--
-- 背景（2026-11-23 站长指示）：app_vip 档自 0004 种子起上架，但生效链
-- （crates/flux-shared lib.rs 的 "vip" | "app_vip" 臂）与「贵宾待遇」完全同源
-- ——都是写 users.vip_until，站内没有任何「APP 专属」的独立权益位。
-- 档名卖的是一个不存在的差异化能力（与 0295 下架「15 天去广告」同理）。
--
-- 修法：置 active = false。商城目录端点（economy_http/shop_list.rs）只列
-- active 行，前台即刻消失；后台「道具管理」仍能看到这一档，站长随时可以
-- 手动置回 active 恢复售卖（恢复前应先把 APP 侧的专属权益真正落地）。
--
-- 不物理删（同 0207 / 0295 口径）：
--   * shop_orders.item_id 外键引用，删行会炸历史订单；
--   * 已购用户的 vip_until 保留——下架只停新售，不追回已生效的时长。
--
-- 按 kind 而非 id 圈行：shop_items.kind 无唯一约束，后台新建的同 kind 行
-- 一并下架，避免只关种子行留一条漏网。
-- 幂等：WHERE active，重复执行 0 行。

UPDATE shop_items SET active = false
WHERE kind = 'app_vip' AND active;
