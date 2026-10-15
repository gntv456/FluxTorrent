-- 0381 考核多阶段 + 失败补救期（PT Studio FR-EXAM-001~017 对标）
--
-- 现状（0093 / task_claims 单表）：一个 task = 一段时限 + 一组指标，
-- 到期即判「完成 / 失败」二选一。对照 NexusPHP / PT Studio 的成熟形态，
-- 缺三样：
--
--   1. **多阶段**：一个考核拆成若干阶段（每阶段自己的时限与目标），
--      逐阶段推进而非一次定生死。现实现只有单段，考核期长时用户早期
--      落后就彻底出局，中途没有「还剩几步」的可见度。
--   2. **补救期（remedy）**：到期未达标先进入宽限期，期间补齐即算通过。
--      现实现到期直接失败——用户出差一周回来就被封，申诉成本高、
--      站点也白白损失一个已接近达标的用户。
--   3. **阶段级基线**：每阶段有自己的起点快照。现实现只有认领时的
--      单点 base，多阶段下无法表达「第 2 阶段从第 1 阶段结束时的
--      位置起算」。
--
-- 【向后兼容】tasks / task_claims 全部保留原语义：
--   * 未配置阶段的 task 走旧路径（阶段数 = 1，remedy_days = 0）；
--   * task_claims 新列全部可空，老行不填即旧行为。
-- worker 的 settle 循环按 stages 逐段推进，无阶段配置时与现状逐位一致。

-- ① 阶段定义：一个 task 可配 N 个阶段，按 seq 升序
CREATE TABLE IF NOT EXISTS exam_stages (
    task_id      BIGINT      NOT NULL REFERENCES tasks(id) ON DELETE CASCADE,
    seq          INT         NOT NULL,
    name         TEXT        NOT NULL,
    -- 本阶段时限（从本阶段开始算）
    duration_days INT        NOT NULL,
    -- 本阶段目标（metric JSON，语义同 tasks.metric）
    metric       JSONB       NOT NULL DEFAULT '{}'::jsonb,
    -- 本阶段奖励（0 = 沿用 tasks.reward；多阶段时通常首阶段给引导奖励）
    reward       BIGINT      NOT NULL DEFAULT 0,
    -- 失败补救期天数：到期未达标后额外给的宽限天数（0 = 无补救）
    remedy_days  INT         NOT NULL DEFAULT 0,
    -- 未达标时的处置：none=不处置 / restrict=限制下载 / ban=封禁
    fail_action  TEXT        NOT NULL DEFAULT 'none'
                            CHECK (fail_action IN ('none', 'restrict', 'ban')),
    -- 本阶段延期上限（用户可申请延期的次数）
    max_extensions INT       NOT NULL DEFAULT 0,
    PRIMARY KEY (task_id, seq)
);

COMMENT ON TABLE exam_stages IS
    '考核阶段配置（FR-EXAM 多阶段）。未配置阶段的 task 走旧单段路径，行为不变。';
COMMENT ON COLUMN exam_stages.metric IS
    '本阶段目标，语义同 tasks.metric（upload_delta/seed_seconds_delta/uploads/subtitles…），键出现即参与判定，全部满足才算达标。';
COMMENT ON COLUMN exam_stages.remedy_days IS
    '失败补救期：本阶段到期未达标后额外给的宽限天数，期间补齐即自动通过并视为达标（不计失败）。这是「到期直接封」的反面——给用户一条不用申诉的路。';

-- ② 认领的阶段推进状态（在 task_claims 上加，不另起表——
--    阶段进度与认领是 1:1，跟着走省一次 JOIN 与一处状态漂移）
ALTER TABLE task_claims
    ADD COLUMN IF NOT EXISTS exam_stage_seq INT NOT NULL DEFAULT 1,
    ADD COLUMN IF NOT EXISTS exam_stage_started_at TIMESTAMPTZ,
    ADD COLUMN IF NOT EXISTS exam_remedy_until TIMESTAMPTZ,
    ADD COLUMN IF NOT EXISTS exam_extensions INT NOT NULL DEFAULT 0,
    ADD COLUMN IF NOT EXISTS exam_last_stage TINYINT NOT NULL DEFAULT 0;

COMMENT ON COLUMN task_claims.exam_stage_seq IS
    '当前所处阶段 seq（从 1 起）。多阶段达标后递增，单段考核恒为 1。';
COMMENT ON COLUMN task_claims.exam_remedy_until IS
    '补救期截止。非空且尚未到期 = 已判失败但在宽限中；期间补齐自动复活。';
COMMENT ON COLUMN task_claims.exam_last_stage IS
    '1 = 已走到最后一阶段。多阶段考核在最后一阶段达标才算整体通过。';

-- ③ 阶段快照：每阶段开始时记一次基线（第 2 阶段从第 1 阶段末尾起算）
CREATE TABLE IF NOT EXISTS exam_stage_bases (
    claim_id     BIGINT      NOT NULL REFERENCES task_claims(id) ON DELETE CASCADE,
    seq          INT         NOT NULL,
    base_uploaded BIGINT     NOT NULL DEFAULT 0,
    base_seed_seconds BIGINT NOT NULL DEFAULT 0,
    base_uploads BIGINT      NOT NULL DEFAULT 0,
    started_at   TIMESTAMPTZ NOT NULL DEFAULT now(),
    PRIMARY KEY (claim_id, seq)
);

COMMENT ON TABLE exam_stage_bases IS
    '每阶段起算基线。多阶段考核的第 N 阶段从第 N-1 阶段结束时的位置起算，'
    '而不是从认领时的位置起算——否则第 2 阶段会把第 1 阶段的成果重复计入。';

-- ④ 补救期复活日志：谁在什么时候被宽限救回来（运营可查，
--    也用于校准 remedy_days 设多长合适）
CREATE TABLE IF NOT EXISTS exam_remedy_log (
    id          BIGSERIAL PRIMARY KEY,
    claim_id    BIGINT      NOT NULL REFERENCES task_claims(id) ON DELETE CASCADE,
    user_id     BIGINT      NOT NULL,
    stage_seq   INT         NOT NULL,
    -- 逾期天数（超出原时限但未超补救期的天数）
    overdue_days INT        NOT NULL,
    -- 宽限期内补齐的时刻
    rescued_at  TIMESTAMPTZ NOT NULL DEFAULT now()
);

CREATE INDEX IF NOT EXISTS idx_exam_remedy_user
    ON exam_remedy_log (user_id, rescued_at DESC);

COMMENT ON TABLE exam_remedy_log IS
    '补救期复活日志。用于回答「补救期该设几天」——若绝大多数人逾期 1-2 天内补齐，'
    '设 0 天就是纯粹制造申诉；这是用数据而不是感觉定策略。';

-- ⑤ 待审延期申请
CREATE TABLE IF NOT EXISTS exam_extensions (
    id          BIGSERIAL PRIMARY KEY,
    claim_id    BIGINT      NOT NULL REFERENCES task_claims(id) ON DELETE CASCADE,
    user_id     BIGINT      NOT NULL,
    stage_seq   INT         NOT NULL,
    reason      TEXT        NOT NULL,
    -- pending / approved / denied
    status      TEXT        NOT NULL DEFAULT 'pending'
                            CHECK (status IN ('pending', 'approved', 'denied')),
    created_at  TIMESTAMPTZ NOT NULL DEFAULT now(),
    handled_by  BIGINT,
    handled_at  TIMESTAMPTZ
);

CREATE INDEX IF NOT EXISTS idx_exam_extensions_pending
    ON exam_extensions (status, created_at)
    WHERE status = 'pending';

COMMENT ON TABLE exam_extensions IS
    '延期申请。每阶段可用次数受 exam_stages.max_extensions 限制（0 = 不可延期）。';
