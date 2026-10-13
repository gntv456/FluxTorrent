-- 0354 harem 邀请持续分润（做种收益的邀请人分成）。
--
-- 依据（ColaPT harem 口径的直接对应）：被邀请人的做种收益按比例给邀请人
-- 持续分成。原式 min(own/0.2, Σinvitee) × 0.2 ——「分润不超过邀请人自身
-- 产出的同比例」，防纯挂名邀请人不劳而获。代数等价变形（两边同乘 pct%）：
--   min(own/(pct/100), Σ) × (pct/100) = min(own, Σ × pct/100)
-- 本站按变形后形态实现并参数化：
--   pct = seeding_referral_pct（0..=50，百分数；0 = 关闭）
--   每个邀请人每小时分润 = LEAST( Σ(被邀请人 amount) × pct/100,
--                                邀请人自身 amount )
--   pct=20 时与 ColaPT 原式逐值一致（min 保序，乘正数不换位）。
--
-- 落账形态（worker seeding_referral.rs，紧随主结算同事务段）：
--   kind = 'seeding_referral'（**独立 kind**：game_coupons.rs 按
--   kind='seeding_reward' 判定每日领券，混入会污染口径）
--   幂等键 = seeding-inv:{inviter}:{yyyymmddhh}（与主结算的
--   seeding:{user}:{hour} 不同前缀，防串）
--   pct=0 时 SQL 直接短路不产生行。
--
-- 幂等（ON CONFLICT DO NOTHING），重跑安全。

INSERT INTO site_settings (name, value, descr, grp)
VALUES ('seeding_referral_pct', '20',
        '邀请分润比例（%）：被邀请人做种收益按该比例持续分给邀请人，'
        '且分润不超过邀请人自身产出同比例（LEAST 钳制）。0 = 关闭。',
        'economy')
ON CONFLICT (name) DO NOTHING;

INSERT INTO settings_meta
  (name, type, label_zh, label_en, hint, unit, group_key, card_order,
   visible, min, max, step)
VALUES
  ('seeding_referral_pct', 'number', '邀请分润比例', 'Referral seeding share',
   '被邀请人做种收益按该百分比分给邀请人（每小时随做种结算发）。分润额'
   '受 LEAST 钳制：不超过邀请人自身产出，纯挂名邀请人拿不到钱。'
   '设为 0 关闭。',
   '%', 'economy', 47, true, 0, 50, 1)
ON CONFLICT (name) DO NOTHING;
