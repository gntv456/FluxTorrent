-- 种子原始文件存储（M05：下载时重新注入 announce 生成动态 .torrent）
CREATE TABLE torrent_files (
  torrent_id BIGINT PRIMARY KEY REFERENCES torrents(id) ON DELETE CASCADE,
  raw BYTEA NOT NULL,
  created_at TIMESTAMPTZ NOT NULL DEFAULT now()
);
