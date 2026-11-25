-- 0520 固定抽签（2026-11-24）。
--
-- 对标 tangpt.top 的 NexusPHP fixed-lottery 插件：「先报名，再从有效报名者
-- 中抽选固定名额」。与 0126 论坛抽奖（票价入池、赢家分池）不同，本模块是
-- **固定奖项**：每奖项名额 1-1000，奖励类型固定（火花/邀请/上传量/勋章/
-- VIP/免广告/彩虹ID/改名卡/娱乐屋物品/自定义领取），不随报名人数变化。
--
-- 公平性：开奖时生成 32 字节 CSPRNG seed，每个候选人排序键 =
-- HMAC-SHA256(key=seed, msg="{campaign_id}:{uid}")，升序取前 N（bottom-k）。
-- seed 与候选快照哈希落 draw_meta，任何拿到 seed+名单的人可离线复算。
-- 纯函数实现见 domain/fixed_lottery（与 lottery_fair 同风格）。
--
-- 触发：scheduled（定时）/ threshold（人数达标+保底时间）双模式；
-- worker 60s tick 扫到期活动 CAS 开奖，0 人报名记 draw_meta.voided=true 流拍。
--
-- 履约两段式：开奖事务只落 winner(pending)+fulfillment(pending)；奖励发放
-- 由 worker/services 逐个履约（apply_item_effect 非事务版），失败按
-- 1/5/15/60/360 分钟退避重试，耗尽转人工。fulfillment 以 winner_id 为
-- 主键——campaign+winner 天然幂等，重放不双发。
--
-- 报名：PK(campaign_id, user_id) 幂等；ticket_price>0 时报名扣火花
-- （flx-join:{cid}:{uid} 幂等键，纯回收口不进奖池）。

BEGIN;

-- ── 1) 活动 ─────────────────────────────────────────────────────────────
CREATE TABLE IF NOT EXISTS fixed_lottery_campaigns (
    id                  BIGSERIAL PRIMARY KEY,
    participation_code  CHAR(8) NOT NULL UNIQUE,      -- 公开跳转键，不暴露自增 id
    title               TEXT NOT NULL,
    intro               TEXT NOT NULL DEFAULT '',
    status              TEXT NOT NULL DEFAULT 'draft',
    trigger_type        TEXT NOT NULL DEFAULT 'scheduled',
    starts_at           TIMESTAMPTZ,
    scheduled_draw_at   TIMESTAMPTZ,                  -- scheduled 必填
    threshold_count     INTEGER,                      -- threshold 必填 (>=1)
    fallback_draw_at    TIMESTAMPTZ,                  -- threshold 必填且晚于 starts_at
    ticket_price        BIGINT NOT NULL DEFAULT 0,    -- 报名票价（火花），0=免费
    eligibility         JSONB NOT NULL DEFAULT '{}'::jsonb,
    claim_default_days  INTEGER NOT NULL DEFAULT 14,  -- 自定义奖领取期限（天）
    draw_meta           JSONB,                        -- seed/seed_hash/快照哈希/voided
    drawn_at            TIMESTAMPTZ,
    drawn_by            BIGINT,                       -- 手动开奖操作员；auto 为 NULL
    cancel_reason       TEXT,
    cancelled_at        TIMESTAMPTZ,
    created_by          BIGINT NOT NULL,
    created_at          TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at          TIMESTAMPTZ NOT NULL DEFAULT now(),
    CONSTRAINT fl_campaign_status_known
        CHECK (status IN ('draft', 'open', 'drawn', 'cancelled')),
    CONSTRAINT fl_campaign_trigger_known
        CHECK (trigger_type IN ('scheduled', 'threshold'))
);

-- 到期扫描只打 open 行；部分索引兼作 worker 扫描加速。
CREATE INDEX IF NOT EXISTS fl_campaign_due_idx
    ON fixed_lottery_campaigns (scheduled_draw_at)
    WHERE status = 'open';

-- ── 2) 奖项 ─────────────────────────────────────────────────────────────
CREATE TABLE IF NOT EXISTS fixed_lottery_prizes (
    id            BIGSERIAL PRIMARY KEY,
    campaign_id   BIGINT NOT NULL
                  REFERENCES fixed_lottery_campaigns (id) ON DELETE CASCADE,
    sort_order    INTEGER NOT NULL,                 -- 开奖按此序展开名额
    name          TEXT NOT NULL,
    quantity      INTEGER NOT NULL DEFAULT 1,
    reward_kind   TEXT NOT NULL,
    reward_config JSONB NOT NULL DEFAULT '{}'::jsonb,
    CONSTRAINT fl_prize_campaign_sort_unique UNIQUE (campaign_id, sort_order),
    CONSTRAINT fl_prize_qty_range CHECK (quantity BETWEEN 1 AND 1000),
    -- 奖励类型白名单：发放入口逐一有实现，库里挡住手滑键名
    CONSTRAINT fl_prize_reward_known CHECK (reward_kind IN (
        'spark', 'invite', 'item', 'medal', 'upload',
        'vip', 'ad_free', 'rainbow_id', 'rename_card', 'custom'
    ))
);

-- ── 3) 报名 ─────────────────────────────────────────────────────────────
CREATE TABLE IF NOT EXISTS fixed_lottery_participants (
    campaign_id       BIGINT NOT NULL
                      REFERENCES fixed_lottery_campaigns (id) ON DELETE CASCADE,
    user_id           BIGINT NOT NULL REFERENCES users (id) ON DELETE CASCADE,
    username_snapshot TEXT NOT NULL DEFAULT '',
    joined_at         TIMESTAMPTZ NOT NULL DEFAULT now(),
    last_ip           TEXT,
    PRIMARY KEY (campaign_id, user_id)                -- 一人一活动一次报名
);

-- ── 4) 中奖 ─────────────────────────────────────────────────────────────
CREATE TABLE IF NOT EXISTS fixed_lottery_winners (
    id                 BIGSERIAL PRIMARY KEY,
    campaign_id        BIGINT NOT NULL
                       REFERENCES fixed_lottery_campaigns (id) ON DELETE CASCADE,
    prize_id           BIGINT NOT NULL
                       REFERENCES fixed_lottery_prizes (id) ON DELETE CASCADE,
    user_id            BIGINT NOT NULL REFERENCES users (id) ON DELETE CASCADE,
    username_snapshot  TEXT NOT NULL DEFAULT '',
    prize_name_snapshot TEXT NOT NULL DEFAULT '',
    slot_no            INTEGER NOT NULL,              -- 奖项内第几号名额
    draw_position      INTEGER NOT NULL,              -- 全局开奖位次（1 起）
    selection_key      TEXT NOT NULL DEFAULT '',      -- 本人 HMAC 排序键（复算用）
    status             TEXT NOT NULL DEFAULT 'pending',
    claim_deadline_at  TIMESTAMPTZ,                   -- 仅 custom 奖
    created_at         TIMESTAMPTZ NOT NULL DEFAULT now(),
    CONSTRAINT fl_winner_campaign_user_unique UNIQUE (campaign_id, user_id),
    CONSTRAINT fl_winner_campaign_slot_unique
        UNIQUE (campaign_id, prize_id, slot_no),
    CONSTRAINT fl_winner_status_known CHECK (status IN (
        'pending', 'fulfilled', 'satisfied', 'failed', 'claimed'
    ))
);

CREATE INDEX IF NOT EXISTS fl_winner_campaign_order_idx
    ON fixed_lottery_winners (campaign_id, draw_position);

-- ── 5) 履约 ─────────────────────────────────────────────────────────────
-- 每个中奖恰好一条履约记录：campaign_id+winner_id 业务幂等 = winner_id 主键。
CREATE TABLE IF NOT EXISTS fixed_lottery_fulfillments (
    winner_id       BIGINT PRIMARY KEY
                    REFERENCES fixed_lottery_winners (id) ON DELETE CASCADE,
    status          TEXT NOT NULL DEFAULT 'pending',
    attempt_count   INTEGER NOT NULL DEFAULT 0,
    last_error      TEXT,
    next_retry_at   TIMESTAMPTZ,
    reward_snapshot JSONB NOT NULL DEFAULT '{}'::jsonb,
    fulfilled_at    TIMESTAMPTZ,
    updated_at      TIMESTAMPTZ NOT NULL DEFAULT now(),
    CONSTRAINT fl_fulfill_status_known CHECK (status IN (
        'pending', 'fulfilled', 'satisfied', 'failed', 'exhausted'
    ))
);

CREATE INDEX IF NOT EXISTS fl_fulfill_retry_idx
    ON fixed_lottery_fulfillments (next_retry_at)
    WHERE status IN ('pending', 'failed');

-- ── 6) 自定义奖品领取 ───────────────────────────────────────────────────
CREATE TABLE IF NOT EXISTS fixed_lottery_claims (
    winner_id     BIGINT PRIMARY KEY
                  REFERENCES fixed_lottery_winners (id) ON DELETE CASCADE,
    submission    JSONB NOT NULL DEFAULT '{}'::jsonb, -- 动态表单字段值
    status        TEXT NOT NULL DEFAULT 'pending_review',
    reject_reason TEXT,
    submitted_at  TIMESTAMPTZ,
    reviewed_by   BIGINT,
    reviewed_at   TIMESTAMPTZ,
    updated_at    TIMESTAMPTZ NOT NULL DEFAULT now(),
    CONSTRAINT fl_claim_status_known CHECK (status IN (
        'pending_review', 'approved', 'rejected', 'expired'
    ))
);

-- ── 7) 大厅卡片 ─────────────────────────────────────────────────────────

-- ── 8) 管理面板导航行（0241 同款口径） ────────────────────────────────
INSERT INTO staff_panel_entries
    (panel, name, url, info, sort, section, min_class, tab_key,
     module_key, perm_key)
VALUES
    ('admin', '固定抽签', '/admin?tool=flxlottery',
     '活动 CRUD / 立即开奖 / 履约状态 / 领取审核', 66, 'ops', 92,
     'flxlottery', 'games', 'user.adjust')
ON CONFLICT DO NOTHING;

INSERT INTO arcade_games
    (key, title_key, icon, href, tone, grp, module_gate, badge, hot, enabled, sort)
VALUES
    ('lottery', 'lottery', '🎪', '/games/lottery', 'coral',
     'session', '', '', false, true, 25)
ON CONFLICT (key) DO NOTHING;

COMMIT;
