-- 0295：运营轮 P0-2「权益与到期脱钩」
--
-- 事实（读码 + 库实读）：
--   1) users.donor 一旦被写 TRUE，全库没有任何复位路径（无任务、无读时判断）。
--      而站内魔力购买 VIP 的副作用就是 donor = TRUE
--      （economy_http/shop_effects.rs 的 "vip" | "app_vip" 分支）
--      ⇒ 花钱买 30 天待遇，拿到的是永久待遇：做种火花 donor_mult 倍率、
--        魔力×2、免催账全部长期生效，并且人人挂上「捐赠者」徽章。
--   2) users.donor_until（0079 为「免广告」加的子周期）全库只有写入方
--      （shop_effects "ad_free"）与展示方，没有任何判定点读它 ⇒ 买了等于没买。
--   3) 站内没有任何广告渲染，商品「15 天去广告」（shop_items.id=11，10000 魔力）
--      描述的是不存在的能力。
--
-- 口径（本轮拍板）：
--   * donor 只表示「真金白银的捐赠者」——徽章与永久待遇归它，
--     由 payment/settle.rs、staff_http/donate_admin.rs 在真实入账时写入。
--   * 站内魔力购买的待遇一律由到期时间驱动：vip_until（贵宾档）、
--     donor_until（其余特权档）。读时判定，不依赖到期清理任务——
--     这与运营轮 P1-3（清理任务挂在业务模块开关上）是同一个病根：
--     只要判定看时间，模块开关关掉也不会让过期权益继续生效。
--   * 判定点收敛到下面这一个函数：待遇散在 4 处 SQL 里各写各的布尔，
--     是「两份规则必漏一份」的标准现场。

-- 捐赠者待遇是否有效：真捐赠（永久）或任一特权档未过期。
CREATE OR REPLACE FUNCTION donor_privileged(uid bigint) RETURNS boolean
LANGUAGE sql STABLE AS $$
    SELECT COALESCE(
        (SELECT u.donor
                    OR COALESCE(u.vip_until > now(), false)
                    OR COALESCE(u.donor_until > now(), false)
           FROM users u WHERE u.id = uid),
        false
    );
$$;

COMMENT ON FUNCTION donor_privileged(bigint) IS
    '捐赠者待遇判定：donor（真金白银，永久）或 vip_until / donor_until 任一未过期。'
    '读时判定，到期即失效，不需要清理任务。魔力购买 VIP 只写 vip_until。';

-- 是否已持有「可用」券（未核销且未过期）。运营轮 P0-3：
-- 购买侧与发放侧要用同一条判据——此前 shop_effects 的防重 EXISTS 不带
-- used_at / expires_at，于是「曾经有过一张（已用或已过期）」就跳过补发，
-- 结果魔力照扣、券不发；而购买侧根本没有护栏（装扮类有），也就无从报错。
CREATE OR REPLACE FUNCTION has_usable_voucher(uid bigint, vkind text)
RETURNS boolean LANGUAGE sql STABLE AS $$
    SELECT EXISTS (
        SELECT 1 FROM user_vouchers v
        WHERE v.user_id = uid AND v.kind = vkind
          AND v.used_at IS NULL AND v.expires_at > now()
    );
$$;

COMMENT ON FUNCTION has_usable_voucher(bigint, text) IS
    '该用户是否已有可用（未核销、未过期）的某类券。购买拦截与发放防重共用。';

-- 存量回落：donor 只保留有资金痕迹的账号（已支付订单或捐赠流水）。
-- 资金痕迹之外置位的 donor，来源只有上面那条 VIP 分支（以及它的历史版本）。
UPDATE users u
SET donor = false
WHERE u.donor
  AND NOT EXISTS (
      SELECT 1 FROM payment_orders p
      WHERE p.user_id = u.id AND p.status = 'paid'
  )
  AND NOT EXISTS (
      SELECT 1 FROM donation_ledger d WHERE d.user_id = u.id
  );

-- 「15 天去广告」下架：站内没有广告位，这条商品卖的是不存在的能力。
-- 要恢复先把广告渲染落地并用 donor_privileged() 门控，再置回 active。
UPDATE shop_items SET active = false
WHERE kind = 'ad_free' AND active;
