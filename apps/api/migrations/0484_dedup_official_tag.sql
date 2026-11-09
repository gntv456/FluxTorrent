-- 0484_dedup_official_tag.sql — official 标签去重 + 唯一性约束
--
-- 背景（2026-11-09）：种子标签体系允许 kind='official' 的官方/官种标签，但
-- 一度出现两条：id=1「官方」(迁移官种数据时手工补建) 与点站原生的 id=3
-- 「官种」(official)。两者语义重叠，导致种子详情页标签云并列显示两个蓝色
-- official 标签（"两个官方标签"），且头部徽章文案是「官种」。已在线把 19977
-- 条关联从「官方」合并到「官种」并删 id=1。
--
-- 本迁移把这次清理固化为幂等步骤，并加唯一索引防复发：
--   1) 若 tag_dict 存在多条 kind='official' & scope='torrent' 行，保留规范行
--      （优先 name='官种'，否则 id 最小者），把其余行的 tags 关联并入规范行后
--      删除其余行；再以 refresh_official_tag 重算终态。
--   2) 对 (kind) 建部分唯一索引（仅 official & scope='torrent'），未来任何试图
--      再建第二个 official 字典行的写入都会被 DB 拒绝。

BEGIN;

-- 1) 合并重复的 official 行（幂等：≤1 行时全部空操作）
WITH officials AS (
    SELECT id, name,
           row_number() OVER (
               ORDER BY (name = '官种') DESC, id ASC
           ) AS rn
      FROM tag_dict
     WHERE kind = 'official' AND scope = 'torrent'
), keep AS (
    SELECT id FROM officials WHERE rn = 1
), drop AS (
    SELECT id FROM officials WHERE rn > 1
), keep_id AS (
    SELECT id FROM keep
)
-- 把待删行的标签关联并入保留行（冲突即跳过）
INSERT INTO tags (torrent_id, tag_id)
SELECT tt.torrent_id, (SELECT id FROM keep_id)
  FROM tags tt
 WHERE tt.tag_id IN (SELECT id FROM drop)
ON CONFLICT (torrent_id, tag_id) DO NOTHING;

-- 删除待删行的残留关联，再删其字典行（FK 级联也会兜底）
DELETE FROM tags WHERE tag_id IN (
    SELECT id FROM (
        SELECT id, row_number() OVER (ORDER BY (name='官种') DESC, id ASC) AS rn
          FROM tag_dict WHERE kind='official' AND scope='torrent'
    ) x WHERE rn > 1
);
DELETE FROM tag_dict WHERE id IN (
    SELECT id FROM (
        SELECT id, row_number() OVER (ORDER BY (name='官种') DESC, id ASC) AS rn
          FROM tag_dict WHERE kind='official' AND scope='torrent'
    ) x WHERE rn > 1
);

COMMIT;

-- 2) 重算官种物化列（事务外，refresh 内部自行管理）
SELECT refresh_official_tag(NULL);

-- 3) 唯一索引：一个 scope='torrent' 下至多一个 official 字典行
CREATE UNIQUE INDEX IF NOT EXISTS uq_tag_dict_official
    ON tag_dict (kind)
 WHERE kind = 'official' AND scope = 'torrent';
