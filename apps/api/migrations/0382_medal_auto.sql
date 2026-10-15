-- 0382 勋章自动成就（PT Studio FR-MEDAL-001~022 对标）
--
-- 现状：medals 表只有名字/价格/稀有度，user_medals 的 source 只有
-- 'buy'（+gift 等）。授予完全是**手动**的——后台点一下才算数。
--
-- 缺三样（评估报告 §2.7）：
--   1. **双模式成就**：事件计数（做种满 100 小时 → 自动授「勤勉」）+
--      统计阈值（上传量/做种时长/注册天数达线 → 自动授）；
--   2. **回填**：新建一个成就后，能对存量用户全量评估；
--      事件计数类还要能从历史事件重放（否则新成就永远只对新用户有效）；
--   3. **进度可见**：当前值/目标（前端「还差多少」）。
--
-- 【为什么事件计数要有水位线而不是无限累加】
-- 直接累加 events 行会把 events 表变成全站最热的表，且回放时
-- 「重启服务前的进度」无处可取。故落 `medal_progress`（每人每成就一行），
-- 事件只增这一个数；历史回放走 domain 层判定 + 一次性批量写。

-- ① 成就定义（挂在 medals 上：自动与手动共用一张表，
--    避免「自动勋章不是勋章」导致两套佩戴/展示逻辑）
ALTER TABLE medals
    ADD COLUMN IF NOT EXISTS auto_rule TEXT,
    ADD COLUMN IF NOT EXISTS auto_mode TEXT,
    ADD COLUMN IF NOT EXISTS auto_target BIGINT,
    ADD COLUMN IF NOT EXISTS auto_period TEXT,
    ADD COLUMN IF NOT EXISTS progress_visible BOOLEAN NOT NULL DEFAULT false,
    ADD COLUMN IF NOT EXISTS backfill_done BOOLEAN NOT NULL DEFAULT false,
    ADD COLUMN IF NOT EXISTS sort_order INT NOT NULL DEFAULT 0;

COMMENT ON COLUMN medals.auto_mode IS
    'NULL = 手动授予（非成就）。event = 事件计数（做种/发布/评论…）统计累加；threshold = 统计阈值（上传量/注册天数）周期评估。';
COMMENT ON COLUMN medals.auto_target IS
    '达成阈值。event 模式为累计次数；threshold 模式为统计值。';
COMMENT ON COLUMN medals.auto_period IS
    'threshold 模式的统计周期：total=累计 / day=日 / week=周 / month=月。与 domain::medal 的取数口径对应。';
COMMENT ON COLUMN medals.backfill_done IS
    '1 = 已对该成就做过一次存量回填。幂等闸门：回填任务靠它避免每轮重扫全站。';

-- ② 进度：每人每成就一行。事件计数模式的累加落点。
CREATE TABLE IF NOT EXISTS medal_progress (
    user_id     BIGINT      NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    medal_id    BIGINT      NOT NULL REFERENCES medals(id) ON DELETE CASCADE,
    current     BIGINT      NOT NULL DEFAULT 0,
    awarded_at  TIMESTAMPTZ,
    updated_at  TIMESTAMPTZ NOT NULL DEFAULT now(),
    PRIMARY KEY (user_id, medal_id)
);

CREATE INDEX IF NOT EXISTS idx_medal_progress_unawarded
    ON medal_progress (medal_id, current)
    WHERE awarded_at IS NULL;

COMMENT ON TABLE medal_progress IS
    '成就进度（当前值/授予时刻）。事件计数模式累加 current；threshold 模式按周期整体重算。awarded_at 非空 = 已授予。';

-- ③ 事件计数：需要从历史事件回放，故留事件流（只记可计数的那几类）
CREATE TABLE IF NOT EXISTS medal_events (
    id       BIGSERIAL PRIMARY KEY,
    user_id  BIGINT NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    event    TEXT   NOT NULL,
    amount   BIGINT NOT NULL DEFAULT 1,
    at       TIMESTAMPTZ NOT NULL DEFAULT now()
);

CREATE INDEX IF NOT EXISTS idx_medal_events_user_event
    ON medal_events (user_id, event);
CREATE INDEX IF NOT EXISTS idx_medal_events_at
    ON medal_events (at);

COMMENT ON TABLE medal_events IS
    '可计数事件流（FR-MEDAL 回放源）。只收「成就语义明确」的类别——'
    '不收全部领域事件：全量事件表会成为全站最热的写表，而成就只需要'
    '计数口径明确的那几类。amount 让「一次做种 10 小时」能按量累加。';

-- ④ 授予幂等：并发下两个 worker 同时判定达标也只发一次
CREATE UNIQUE INDEX IF NOT EXISTS idx_user_medals_auto
    ON user_medals (user_id, medal_id)
    WHERE source IN ('event', 'threshold');

COMMENT ON INDEX idx_user_medals_auto IS
    '自动授予的幂等闸门：同一人同一成就只会有一行，自动模式与手动买'
    '（source=buy）互不冲突——买了也自动达成不会双发。';
