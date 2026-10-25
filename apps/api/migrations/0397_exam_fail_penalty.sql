-- 0397 考核不达标处置菜单（fail_penalty JSONB，多选+参数化）
--
-- 现状（0381）：exam_stages.fail_action 三态枚举（none/restrict/ban），
-- 且只有配置了 exam_stages 的多阶段路径才有处置；单段考核（绝大多数站
-- 的常态——0093 新人转正考核就是单段）到期只 settle_fail 扣罚金，
-- 站长没有任何处置手段可选。
--
-- 本迁移把处置升级为**可多选的惩罚菜单**（站长勾选组合 + 每项参数）：
--   ban              封禁账号（status=2，与后台人工封禁同口径）
--   suspend          挂起（suspended=TRUE，下载计费闸拦截）
--   restrict_download 禁下载（download_enabled=FALSE + download_locked_by）
--   mute             禁言（status=1 + forumpost=FALSE）
--   ban_upload       禁发布（user_permissions 显式拒绝 torrent.upload）
--   ban_pages        禁浏览指定页面（user_permissions 显式拒绝页面权限位）
--   spark_fine       扣魔力（spark_ledger 流水驱动，只扣到 0 不负余额）
--   traffic_fine     扣上传量/下载量（GREATEST(col-amount,0)，不负）
--
-- 【为什么 JSONB 而不是枚举加列】处置是「组合」不是「单选」——封禁+扣魔力、
-- 禁言+禁下载都是合理搭配。枚举列表达不了组合；每组合加一列则 8 项 = 2^8 列。
-- JSONB 数组 + 每项可选参数对象，菜单以后加项不改表。
--
-- 【与 fail_action 的关系】fail_action 保留（0381 的多阶段状态机判定矩阵
-- 在读它），fail_penalty 为空时回退 fail_action 三态——已有站点零感知。
-- fail_penalty 非空时以它为准（restrict→[restrict_download]、ban→[ban]
-- 的映射关系见 worker penalty.rs）。

-- ① 多阶段：阶段级处置菜单
ALTER TABLE exam_stages
    ADD COLUMN IF NOT EXISTS fail_penalty JSONB NOT NULL DEFAULT '[]'::jsonb;

COMMENT ON COLUMN exam_stages.fail_penalty IS
    '未达标处置菜单（0397，多选+参数化）：[{kind,amount,pages,days}...]。'
    '空数组 = 回退 fail_action 三态（0381 兼容）。'
    'kind ∈ ban/suspend/restrict_download/mute/ban_upload/ban_pages/spark_fine/traffic_fine。';

-- ② 单段考核：任务级处置菜单（0093 单段路径此前无任何处置手段）
ALTER TABLE tasks
    ADD COLUMN IF NOT EXISTS fail_penalty JSONB NOT NULL DEFAULT '[]'::jsonb;

COMMENT ON COLUMN tasks.fail_penalty IS
    '单段考核未达标处置菜单（0397），语义同 exam_stages.fail_penalty。'
    '普通任务（kind=task）不消费此列——只有考核（onboard/periodic）结算时应用。';

-- ③ 处置落账：谁在何时因哪个考核被处置了什么（审计 + 解除依据）
--    不复用 audit_log：那是「管理员动作」语义（actor_id 必填 FK），
--    自动处置的 actor 是系统；且站长需要「当前生效中的处置」视图，
--    从 audit_log 的 JSON 里反查太脆。
CREATE TABLE IF NOT EXISTS exam_penalties (
    id          BIGSERIAL PRIMARY KEY,
    user_id     BIGINT      NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    claim_id    BIGINT      NOT NULL REFERENCES task_claims(id) ON DELETE CASCADE,
    task_id     BIGINT      NOT NULL REFERENCES tasks(id) ON DELETE CASCADE,
    stage_seq   INT         NOT NULL DEFAULT 1,
    -- 惩罚项 kind（见上方菜单）+ 快照参数（金额/页面，处置时刻冻结）
    kind        TEXT        NOT NULL,
    params      JSONB       NOT NULL DEFAULT '{}'::jsonb,
    -- 实际执行量（如 spark_fine 余额不足只扣到 0，实扣≠配置额）
    applied     JSONB       NOT NULL DEFAULT '{}'::jsonb,
    created_at  TIMESTAMPTZ NOT NULL DEFAULT now(),
    -- 幂等：同一认领同一 kind 只处置一次（worker 重跑/并发双跑防护）
    UNIQUE (claim_id, kind)
);

CREATE INDEX IF NOT EXISTS idx_exam_penalties_user
    ON exam_penalties (user_id, created_at DESC);

COMMENT ON TABLE exam_penalties IS
    '考核不达标处置落账（0397）。一行 = 一个惩罚项；同认领同 kind 幂等。'
    '站长解除处置走后台用户管理（与本表口径相同——写同一批 users 列），'
    '解除后本表仍留痕作为处置历史。';
