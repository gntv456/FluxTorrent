-- 0038 管理系统复刻第五轮（好学站 /nexusphp 后台口径）
-- P1-3：下载权限 / 挂起（NP users.enabled / downloadpos 语义，tracker 执行点）
ALTER TABLE users ADD COLUMN IF NOT EXISTS download_enabled BOOLEAN NOT NULL DEFAULT TRUE;  -- 下载权限
ALTER TABLE users ADD COLUMN IF NOT EXISTS suspended BOOLEAN NOT NULL DEFAULT FALSE;        -- 挂起（禁言/冻结）

-- P1-4：种子拒绝原因字典（NP torrent_deny_reasons，审核拒绝下拉）
CREATE TABLE IF NOT EXISTS torrent_deny_reasons (
    id        BIGSERIAL PRIMARY KEY,
    sort      INT NOT NULL DEFAULT 0,
    reason    TEXT NOT NULL,
    enabled   BOOLEAN NOT NULL DEFAULT TRUE,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now()
);
INSERT INTO torrent_deny_reasons (sort, reason) VALUES
  (1, '重复发布（伪原创）'),
  (2, '种子描述不完整或与内容不符'),
  (3, '资源质量不达标'),
  (4, '分类/媒介选择错误'),
  (5, '违规内容'),
  (6, '缺少必要说明（来源/出处）'),
  (7, '其他（见审核备注）')
ON CONFLICT DO NOTHING;

-- P1-4：种子拒绝落库（审核拒绝时引用字典 + 自由备注）
ALTER TABLE torrents ADD COLUMN IF NOT EXISTS deny_reason_id BIGINT REFERENCES torrent_deny_reasons(id);
ALTER TABLE torrents ADD COLUMN IF NOT EXISTS deny_note TEXT;

-- P1-5：种子操作记录（NP torrent_operation_logs：谁/何时/对哪个种子/做了什么）
CREATE TABLE IF NOT EXISTS torrent_operation_logs (
    id         BIGSERIAL PRIMARY KEY,
    torrent_id BIGINT NOT NULL REFERENCES torrents(id) ON DELETE CASCADE,
    operator_id BIGINT REFERENCES users(id),
    action     TEXT NOT NULL,             -- approve/reject/delete/sticky/promote/hr_pardon/edit/...
    detail     JSONB,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now()
);
CREATE INDEX IF NOT EXISTS idx_torrent_op_logs_torrent ON torrent_operation_logs (torrent_id, id DESC);
CREATE INDEX IF NOT EXISTS idx_torrent_op_logs_time ON torrent_operation_logs (created_at DESC);
