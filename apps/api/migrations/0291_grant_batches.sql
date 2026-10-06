-- 0291 发放台账（道具发放面实测落地）
--
-- 此前批量发放只在 audit_log 里留一条 ref，且 ref 只带前 20 个目标 id 抽样
-- （increment_bulk.rs 的 sample_targets）。站长问「昨天那批 10TB 发到谁了、
-- 有没有漏发」，界面上无人能答；`/admin/audit` 又硬顶 200 行、不渲染 ref。
--
-- 这张表同时补两件事：
--   1. target_ids 全量留档（可按批次回放受众，漏发核对有依据）；
--   2. idempotency_key 唯一约束——幂等在 INSERT 上撞约束，不是
--      SELECT-then-INSERT（后者并发双击两道都能过，实测同参数连发两次
--      魔力 +200，就是双批真发）。
-- actor_id 不设 ON DELETE：账号是墓碑化删除（users 行留存），
-- 台账归属必须永久可追；若哪天有人物理删号，让它报错而不是静默丢留证。

CREATE TABLE grant_batches (
    id bigserial PRIMARY KEY,
    batch_id text NOT NULL UNIQUE,
    idempotency_key text UNIQUE,
    actor_id bigint NOT NULL REFERENCES users(id),
    -- spark | uploaded | invite | resub_card | medal | item
    kind text NOT NULL,
    amount bigint NOT NULL,
    item_id bigint REFERENCES shop_items(id) ON DELETE SET NULL,
    medal_id bigint REFERENCES medals(id) ON DELETE SET NULL,
    days integer,
    -- 受众口径（classes / roles / user_ids），回答「当时圈的是谁」
    selector jsonb NOT NULL DEFAULT '{}'::jsonb,
    target_ids bigint[] NOT NULL DEFAULT '{}',
    affected integer NOT NULL DEFAULT 0,
    -- pending: 已占位未执行（进程崩在这里也留得下这批）
    -- partial: 分批执行中某一批失败，前面几批已真实发出
    status text NOT NULL DEFAULT 'pending'
        CHECK (status IN ('pending', 'done', 'partial', 'failed')),
    error text,
    subject text,
    created_at timestamptz NOT NULL DEFAULT now()
);

CREATE INDEX grant_batches_actor_idx ON grant_batches (actor_id, id DESC);
CREATE INDEX grant_batches_kind_idx ON grant_batches (kind, id DESC);
CREATE INDEX grant_batches_time_idx ON grant_batches (created_at DESC);
