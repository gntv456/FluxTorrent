-- 0450: 作品追更订阅（站长点名要「用户追更，新内容推给他」）
--
-- 粒度是**作品**而不是种子：作品键 WK（torrent_http/works_sql.rs 里那条
-- 表达式，页面上「同一部作品的多颗种子」就是它聚合出来的）才是用户心里的
-- 「我在追这部剧」。所以订阅表只存 user_id + work_key，不存种子 id ——
-- 存种子的话，新一集发布时订阅就自动失效了，等于没订阅。
--
-- 为什么用 work_key 字符串而不是外键：WK 是若干列拼出来的表达式，没有实体
-- 表可引用；字符串还能让「订阅时作品还没有可看种子、之后才有」成立。
--
-- 派发时机：种子过审（approval_status 0→1）时，在同一处副作用里算 WK、
-- 给订阅者发站内信（kind = work_update）。走 mailer_notify 的偏好过滤，
-- 用户可在「通知偏好」里关掉；同一用户同一作品 24h 内只推一条，防刷屏。

CREATE TABLE IF NOT EXISTS work_subscriptions (
    user_id      BIGINT      NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    work_key     TEXT        NOT NULL,
    created_at   TIMESTAMPTZ NOT NULL DEFAULT now(),
    -- 最近一次为该作品推送的时间（24h 节流用）
    notified_at  TIMESTAMPTZ,
    PRIMARY KEY (user_id, work_key)
);

-- 派发侧按 work_key 反查订阅者
CREATE INDEX IF NOT EXISTS idx_work_subs_key
    ON work_subscriptions (work_key);

-- 我的追更列表
CREATE INDEX IF NOT EXISTS idx_work_subs_user
    ON work_subscriptions (user_id, created_at DESC);
