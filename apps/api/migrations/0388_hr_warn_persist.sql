-- 0388_hr_warn_persist.sql
-- H&R 警告阶梯的落库（domain::hr_warn 此前是「有判定、无载体」的孤儿模块）
--
-- 现状（worker `hr.rs::hr_punish`）：未解决违规数 ≥ 阈值（默认 3）
-- → 直接 `download_enabled = false`，全站一个动作，没有轻重。
-- `domain/hr_warn` 把 Gazelle 式的四级阶梯（警告 → 限下 → 禁传 →
-- 禁邀）连同顺延规则都写好了并测过，但**没有任何调用方**：
-- 判定不入库、不参与处置，站点实际行为仍是「三次就禁下」。
--
-- 本迁移给判定一个载体，使阶梯能真正落地：
--   * 每个用户一行「当前警告」：档位、到期时刻、顺延量、是否撞顶；
--   * 到期自动恢复的判据落在 expires_at 上，worker 顺延扫；
--   * 撞顶（hit_cap）单独留痕——顺延封顶意味着「公式已无能为力」，
--     该交人工，按设计不该让它静悄悄封顶。
--
-- 【不改既有处置】hr_punish 的「超阈值禁下」保持原样，本表只是
-- 在其**之前**加一层更轻的阶梯：第 1 档只警告、第 2 档起才限下。
-- 这样做的理由是原来「三次就禁下」对轻度违规过重，且没有任何
-- 「什么时候能恢复」的承诺——本表的 expires_at 给了这个承诺。

BEGIN;

-- ① 用户当前警告态（一行一人）
CREATE TABLE IF NOT EXISTS hr_warn_states (
    user_id      BIGINT      PRIMARY KEY REFERENCES users(id) ON DELETE CASCADE,
    -- 当前档位序号：0=无警告 1..4=阶梯档（对应 LADDER 下标+1）
    step         INT         NOT NULL DEFAULT 0 CHECK (step BETWEEN 0 AND 4),
    -- 本档基准天数（站长可配，写时快照）
    base_days    INT         NOT NULL DEFAULT 7,
    -- 到期时刻：阶梯判定算出来的最终到期（Gazelle 顺延后的结果）
    expires_at   TIMESTAMPTZ,
    -- 本次顺延了多少天（0 = 从今天起算）
    shifted_days INT         NOT NULL DEFAULT 0,
    -- 是否撞到顺延上限——撞到就该人工介入了
    hit_cap      BOOLEAN     NOT NULL DEFAULT false,
    -- 处置动作位：与 LADDER 的三个开关对齐，落库便于读面解释
    restrict_download BOOLEAN NOT NULL DEFAULT false,
    block_upload      BOOLEAN NOT NULL DEFAULT false,
    block_invite      BOOLEAN NOT NULL DEFAULT false,
    updated_at   TIMESTAMPTZ NOT NULL DEFAULT now(),
    -- step>0 必须有到期时刻：没有到期就没有「自动恢复」，等于永久处罚
    CHECK (step = 0 OR expires_at IS NOT NULL)
);

COMMENT ON TABLE hr_warn_states IS
    'H&R 警告阶梯的当前态（0388）。此前判定在 domain::hr_warn 但无载体，'
    '站点实际行为是「未解决违规数 >= 阈值就禁下」——没有轻重、没有恢复承诺。'
    '本表让阶梯与顺延真正落地：expires_at 给出「什么时候自动恢复」这个'
    '此前根本不存在的信息。';

-- 到期恢复的扫描口：只索引「仍有警告且已到期」的行，不扫全表
CREATE INDEX IF NOT EXISTS idx_hr_warn_states_due
    ON hr_warn_states (expires_at)
    WHERE step > 0 AND expires_at IS NOT NULL;

COMMENT ON INDEX idx_hr_warn_states_due IS
    '到期可恢复的警告行（部分索引）。worker 每轮扫这一小撮即可。';

-- ② 警告历史：每次落一条，供申诉与排查「为什么这次比上次久」
CREATE TABLE IF NOT EXISTS hr_warn_history (
    id          BIGSERIAL   PRIMARY KEY,
    user_id     BIGINT      NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    step        INT         NOT NULL,
    base_days   INT         NOT NULL,
    expires_at  TIMESTAMPTZ NOT NULL,
    shifted_days INT        NOT NULL DEFAULT 0,
    hit_cap     BOOLEAN     NOT NULL DEFAULT false,
    note        TEXT        NOT NULL DEFAULT '',
    created_at  TIMESTAMPTZ NOT NULL DEFAULT now()
);

CREATE INDEX IF NOT EXISTS idx_hr_warn_history_user
    ON hr_warn_history (user_id, created_at DESC);

-- ③ 站点参数：基准天数与顺延上限
--    顺延上限默认 0 = 不封顶（与 domain 判据一致：配了才生效）。
--    这里给 365：反复违规推到一年多就人工介入，避免「自动无期」。
INSERT INTO site_settings (name, value) VALUES
    ('hr_warn_base_days', '7'),
    ('hr_warn_max_shift_days', '365'),
    ('hr_warn_enable', '1')
ON CONFLICT (name) DO NOTHING;

COMMENT ON TABLE site_settings IS
    '站点配置。hr_warn_base_days：警告阶梯基准天数（被阶梯按档覆盖）。'
    'hr_warn_max_shift_days：Gazelle 式顺延上限天数（0=不封顶，建议配值）。'
    'hr_warn_enable：总开关（关掉即回到旧的「超阈值禁下」单档行为）。';

COMMIT;
