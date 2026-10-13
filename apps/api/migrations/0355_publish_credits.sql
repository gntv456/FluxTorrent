-- 0355 发布信用分免审（publish_credits，第五条免审通道）。
--
-- 依据：既有四条免审通道（分类 auto_approve / 连击 ≥5 / 等级 ≥阈值 /
-- perm TORRENT_APPROVAL_AUTO）都是「身份或分类」维度，普通用户发好种
-- 仍要每次等审。信用分补「历史行为」维度：稳定产出高质量种的发布者
-- 免审。与 approve_streak（连击，被拒即清零）/ deny_count（累计被拒，
-- 过审清零）语义并列但**独立累积**——连击被一次被拒打断后，信用分
-- 只 -2 仍有残留，鼓励改过而非一挫到底。
--
-- 信用分规则（口径锁定，review.rs / review_batch.rs 两处同步实现）：
--   * 过审：publish_credits = LEAST(5, publish_credits + 1)   -- 上限 5
--   * 被拒：publish_credits = GREATEST(0, publish_credits - 2) -- 下限 0
--   * 被拒禁发命中（deny_count 达阈值被拦）时信用分清零
--     （与 deny_limit 判定同源；本次仅落规则于注释，拦截面不重复扣）
--   免审：publish_credits ≥ publish_credits_threshold（缺省 5）即免审。
--
-- 幂等（IF NOT EXISTS / ON CONFLICT DO NOTHING），重跑安全。

-- A) 信用分列（CHECK 0..=5，与规则上下限一致）
ALTER TABLE users ADD COLUMN IF NOT EXISTS publish_credits INT NOT NULL DEFAULT 0;
DO $$ BEGIN
    IF NOT EXISTS (
        SELECT 1 FROM pg_constraint WHERE conname = 'users_publish_credits_check'
    ) THEN
        ALTER TABLE users
            ADD CONSTRAINT users_publish_credits_check
            CHECK (publish_credits >= 0 AND publish_credits <= 5);
    END IF;
END $$;

-- B) 免审阈值（达到即免审；publishing 组）
INSERT INTO site_settings (name, value, descr, grp)
VALUES ('publish_credits_threshold', '5',
        '发布信用分免审阈值：publish_credits 达到该值即免审直接过。'
        '信用分规则：过审 +1（上限 5）、被拒 -2（下限 0）。',
        'publishing')
ON CONFLICT (name) DO NOTHING;

INSERT INTO settings_meta
  (name, type, label_zh, label_en, hint, unit, group_key, card_order,
   visible, min, max, step)
VALUES
  ('publish_credits_threshold', 'number', '信用分免审线', 'Publish credits threshold',
   '发布信用分达到该值的用户发种免审。信用分与连击/被拒计数并列独立累积：'
   '过审 +1（上限 5）、被拒 -2（下限 0）。设为 6（超上限）可临时关闭该通道。',
   '分', 'publishing', 1, true, 1, 6, 1)
ON CONFLICT (name) DO NOTHING;
