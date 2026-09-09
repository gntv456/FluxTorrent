-- M24 趣味盒投票（旧站 funvote 口径：投票 +1 火花）
CREATE SEQUENCE IF NOT EXISTS fun_polls_id_seq;
CREATE SEQUENCE IF NOT EXISTS fun_votes_id_seq;

CREATE TABLE IF NOT EXISTS fun_polls (
    id BIGINT PRIMARY KEY DEFAULT nextval('fun_polls_id_seq'),
    question TEXT NOT NULL,
    options JSONB NOT NULL,               -- ["选项A", "选项B", ...]
    closed BOOLEAN NOT NULL DEFAULT FALSE,
    created_by BIGINT REFERENCES users(id),
    created_at TIMESTAMPTZ NOT NULL DEFAULT now()
);

CREATE TABLE IF NOT EXISTS fun_votes (
    id BIGINT PRIMARY KEY DEFAULT nextval('fun_votes_id_seq'),
    poll_id BIGINT NOT NULL REFERENCES fun_polls(id) ON DELETE CASCADE,
    user_id BIGINT NOT NULL REFERENCES users(id),
    option_index INT NOT NULL,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    UNIQUE (poll_id, user_id)             -- 一人一票
);

-- 种子：两道站规级趣味题
INSERT INTO fun_polls (id, question, options, created_by) VALUES
    (1, '深夜下载学习资料时，你的状态更接近？', '["挂着做种去睡觉", "盯着进度条不肯睡", "先评论感谢再睡", "做完种还要挑下一个"]', NULL),
    (2, '如果火花能兑换现实物品，你最想要？', '["一箱咖啡", "机械键盘", "正版教科书", "再多也换不走的做种快乐"]', NULL)
ON CONFLICT (id) DO NOTHING;
