-- 0081 审计修复批次二：info_hash 双口径
-- BEP3 客户端按 info 字典原始字节算 announce 哈希；规范化重编码口径在键序非排序种子上不一致。
-- 存 raw 口径，worker 匹配 OR 双查；存量行回填 = 规范化值（绝大多数种子键本就排序，两者一致）。
ALTER TABLE torrents ADD COLUMN IF NOT EXISTS raw_info_hash character(40);
UPDATE torrents SET raw_info_hash = info_hash WHERE raw_info_hash IS NULL;
CREATE INDEX IF NOT EXISTS idx_torrents_raw_ih ON torrents (raw_info_hash);
