-- 做种里程碑事件（M28 插件系统数据源）：worker 结算时发现达里程碑即插入（幂等）
CREATE TABLE IF NOT EXISTS seed_milestones (
    id BIGINT PRIMARY KEY,
    user_id BIGINT NOT NULL REFERENCES users(id),
    torrent_id BIGINT NOT NULL REFERENCES torrents(id),
    hours INT NOT NULL,             -- 里程碑档位：24/168/720/2160（1天/7天/30天/90天）
    reached_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    UNIQUE (user_id, torrent_id, hours)
);
CREATE SEQUENCE IF NOT EXISTS seed_milestones_id_seq;
