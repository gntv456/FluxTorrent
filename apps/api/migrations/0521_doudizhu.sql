-- 0521 斗地主（AI 房 + 预留 PvP，2026-11-24）。
--
-- 娱乐屋第一款多人牌类玩法。玩家 vs 2 个 AI（mode='ai'）即开即玩；
-- 表结构带 mode 字段预留真人 PvP 房间（'pvp'，本批不实现）。
--
-- 经济口径（游戏域红线 2026-09-19：全部玩法 EV < 1，只回收不增发）：
-- AI 房是「玩家 vs 庄」：开局扣底分（stake），结算按地主倍数定输赢，
-- 赢家派彩走抽水后倍率（rake 10%），静态胜率模型下玩家 EV < 1，
-- 抽水比例走设置键 doudizhu_rake_permille（0..500），代码侧另有硬上限 500。
--
-- 状态机：bidding（叫分）→ playing（出牌）→ settled（已结算）；
-- 完整牌局快照存 state jsonb（三人手牌/底牌/出牌历史/轮转游标），
-- 服务端权威定局——客户端只带动作，不信前端任何牌面数据。
-- 玩家超时不另起 worker：turn_deadline 过期后由下一次任意请求懒托管
-- （AI 代打到底，输赢照算）。

BEGIN;

-- ── 1) 局表：一行一局 ─────────────────────────────────────────────────────
CREATE TABLE IF NOT EXISTS arcade_doudizhu_games (
    id              bigint GENERATED ALWAYS AS IDENTITY PRIMARY KEY,
    user_id         bigint NOT NULL REFERENCES users (id) ON DELETE CASCADE,
    mode            text NOT NULL DEFAULT 'ai'
                    CHECK (mode IN ('ai', 'pvp')),
    status          text NOT NULL DEFAULT 'bidding'
                    CHECK (status IN ('bidding', 'playing', 'settled')),
    stake           bigint NOT NULL CHECK (stake > 0),
    role            text CHECK (role IS NULL OR role IN ('landlord', 'farmer')),
    multiplier      int NOT NULL DEFAULT 1 CHECK (
                        multiplier >= 1 AND multiplier <= 8),
    bomb_count      int NOT NULL DEFAULT 0 CHECK (bomb_count >= 0),
    spring          boolean NOT NULL DEFAULT false,
    result          text CHECK (result IS NULL
                        OR result IN ('win', 'lose')),
    payout          bigint NOT NULL DEFAULT 0 CHECK (payout >= 0),
    net             bigint NOT NULL DEFAULT 0,
    idempotency_key text NOT NULL UNIQUE,
    state           jsonb NOT NULL,
    turn_deadline   timestamptz,
    settled_at      timestamptz,
    created_at      timestamptz NOT NULL DEFAULT now(),
    updated_at      timestamptz NOT NULL DEFAULT now()
);

COMMENT ON TABLE arcade_doudizhu_games IS
    '斗地主对局：一行一局；state jsonb 存服务端权威牌局快照；bidding 叫分 → playing 出牌 → settled 结算；超时懒托管看 turn_deadline';

CREATE INDEX IF NOT EXISTS idx_dz_games_user
    ON arcade_doudizhu_games (user_id, created_at DESC);
CREATE INDEX IF NOT EXISTS idx_dz_games_status
    ON arcade_doudizhu_games (status, updated_at);

-- 每人同时只允许一局活跃（bidding/playing 都算）：并发开局第二行被拒，
-- HTTP 层据此回落为读已存在的活跃局。
CREATE UNIQUE INDEX IF NOT EXISTS uq_dz_games_active
    ON arcade_doudizhu_games (user_id)
    WHERE status IN ('bidding', 'playing');

-- ── 2) 大厅玩法清单 ───────────────────────────────────────────────────────
INSERT INTO arcade_games
    (key, title_key, icon, href, tone, grp, module_gate, badge, hot,
     enabled, sort)
VALUES
    ('doudizhu', 'doudizhu', '🃏', '/games/doudizhu', 'gold', 'session', '',
     '', false, true, 42)
ON CONFLICT (key) DO UPDATE SET
    title_key = EXCLUDED.title_key,
    icon = EXCLUDED.icon,
    href = EXCLUDED.href,
    tone = EXCLUDED.tone,
    grp = EXCLUDED.grp,
    module_gate = EXCLUDED.module_gate,
    badge = EXCLUDED.badge,
    hot = EXCLUDED.hot,
    enabled = EXCLUDED.enabled,
    sort = EXCLUDED.sort,
    updated_at = now();

-- ── 3) 设置键：门槛 + 节奏 + 抽水 ──────────────────────────────────────────
INSERT INTO site_settings (name, value, descr, grp) VALUES
    ('games_minclass_doudizhu', '0', '斗地主最低等级', 'games'),
    ('doudizhu_turn_seconds', '20', '斗地主出牌倒计时（秒）', 'module_fun'),
    ('doudizhu_bid_seconds', '15', '斗地主叫分倒计时（秒）', 'module_fun'),
    ('doudizhu_rake_permille', '100', '斗地主派彩抽水（千分比）', 'module_fun')
ON CONFLICT (name) DO NOTHING;

INSERT INTO settings_meta (name, type, label_zh, label_en, unit, min, max,
                           hint, group_key, card_order) VALUES
    ('games_minclass_doudizhu', 'classlevel', '斗地主等级门槛',
     'Doudizhu min class', NULL, NULL, NULL,
     '达到该等级才能玩；0 = 不限。低于门槛的用户看不到入口，直接调 API 也会被拒',
     '娱乐屋', 11),
    ('doudizhu_turn_seconds', 'number', '出牌倒计时',
     'Doudizhu turn countdown', '秒', 10, 60, NULL, 'module_fun', 18),
    ('doudizhu_bid_seconds', 'number', '叫分倒计时',
     'Doudizhu bid countdown', '秒', 5, 60, NULL, 'module_fun', 19),
    ('doudizhu_rake_permille', 'number', '派彩抽水',
     'Doudizhu payout rake', '‰', 0, 500, NULL, 'module_fun', 20)
ON CONFLICT (name) DO UPDATE
  SET label_zh = EXCLUDED.label_zh, label_en = EXCLUDED.label_en,
      unit = EXCLUDED.unit, min = EXCLUDED.min, max = EXCLUDED.max,
      hint = EXCLUDED.hint,
      group_key = EXCLUDED.group_key, card_order = EXCLUDED.card_order;

COMMIT;
