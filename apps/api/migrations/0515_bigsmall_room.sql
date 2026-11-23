-- 0515: 猜大小「倒计时房间」制（期次 + 本期下注）。
--
-- 此前猜大小是点击即开的单局：一次下注当场掷骰结算。本迁移把玩法改成房间制：
-- 每期一个递增期号，下注窗口 N 秒（设置键 bigsmall_round_seconds，缺省 30），
-- 窗口内可对大/小下注；到期由**懒结算**推进——下一次访问房间时，把已过期的
-- 当前期掷骰、按期内下注统一派彩，再开出下一期。不另起 worker 定时任务：
-- 房间没有玩家时不必空跑开奖，结算仍走事务 + 幂等键，口径与即时局一致。
--
-- 只新增表与设置键，不改 arcade_pool_entries（赔率仍住在 bigsmall_default 行表）；
-- 旧即时接口 /games/bigsmall 保留兼容，房间口是 /games/bigsmall/room/*。

BEGIN;

-- 期次：一行就是一局可下注的房间。status=betting 时下注，settled 后回填骰面与分区。
CREATE TABLE IF NOT EXISTS arcade_bigsmall_rounds (
    id           bigint GENERATED ALWAYS AS IDENTITY PRIMARY KEY,
    status       text NOT NULL DEFAULT 'betting'
                 CHECK (status IN ('betting', 'settled')),
    dice         smallint[3],
    sum          smallint,
    region       text CHECK (region IS NULL OR region IN ('small', 'big', 'triple')),
    total_small  bigint NOT NULL DEFAULT 0 CHECK (total_small >= 0),
    total_big    bigint NOT NULL DEFAULT 0 CHECK (total_big >= 0),
    opened_at    timestamptz NOT NULL DEFAULT now(),
    settled_at   timestamptz,
    created_at   timestamptz NOT NULL DEFAULT now()
);

CREATE INDEX IF NOT EXISTS idx_bs_rounds_status
    ON arcade_bigsmall_rounds (status, opened_at);

-- 本期下注：一笔下注一行。结算后回填 payout（派彩魔力）与 net（派彩 - 注额）。
CREATE TABLE IF NOT EXISTS arcade_bigsmall_bets (
    id                bigint GENERATED ALWAYS AS IDENTITY PRIMARY KEY,
    round_id          bigint NOT NULL
                      REFERENCES arcade_bigsmall_rounds (id) ON DELETE CASCADE,
    user_id           bigint NOT NULL REFERENCES users (id) ON DELETE CASCADE,
    side              text NOT NULL CHECK (side IN ('small', 'big')),
    amount            bigint NOT NULL CHECK (amount > 0),
    idempotency_key   text NOT NULL UNIQUE,
    payout            bigint NOT NULL DEFAULT 0 CHECK (payout >= 0),
    net               bigint NOT NULL DEFAULT 0,
    created_at        timestamptz NOT NULL DEFAULT now()
);

CREATE INDEX IF NOT EXISTS idx_bs_bets_round
    ON arcade_bigsmall_bets (round_id);
CREATE INDEX IF NOT EXISTS idx_bs_bets_user
    ON arcade_bigsmall_bets (user_id, created_at DESC);

-- 全站同一时刻只允许一个 betting 期：懒结算并发下第二个请求若抢着插新期，
-- 被这条部分唯一索引拒掉，回退到读已提交的当前期；settled 期不受约束。
CREATE UNIQUE INDEX IF NOT EXISTS uq_bs_rounds_open
    ON arcade_bigsmall_rounds (status) WHERE status = 'betting';

COMMENT ON TABLE arcade_bigsmall_rounds IS
    '猜大小房间期次：betting 期内下注，到期懒结算掷骰并回填分区；期号自增';
COMMENT ON TABLE arcade_bigsmall_bets IS
    '猜大小房间本期下注：结算后回填 payout/net；幂等键防网络重试重复扣款';

-- 下注窗口长度（秒）。范围 10..120；缺省 30。
INSERT INTO site_settings (name, value, descr, grp) VALUES
    ('bigsmall_round_seconds', '30', '猜大小每期下注倒计时（秒）', 'module_fun')
ON CONFLICT (name) DO NOTHING;

INSERT INTO settings_meta (name, type, label_zh, label_en, unit, min, max,
                           group_key, card_order) VALUES
    ('bigsmall_round_seconds', 'number', '每期下注倒计时',
     'Bigsmall round countdown', '秒', 10, 120, 'module_fun', 17)
ON CONFLICT (name) DO UPDATE
  SET label_zh = EXCLUDED.label_zh, label_en = EXCLUDED.label_en,
      unit = EXCLUDED.unit, min = EXCLUDED.min, max = EXCLUDED.max,
      group_key = EXCLUDED.group_key, card_order = EXCLUDED.card_order;

COMMIT;
