-- 0452: section_dict 补 (kind, name) 唯一约束——两代词表共存的断根件
--
-- 普查背景（2026-11-02 站长可变实体影响面调查，缺陷 #5）：
-- section_dict 只有主键 id，同维度同名词条没有任何 DB 级约束。管理端
-- 新增有应用层查重（section_dict.rs 的 lower(trim(name)) 查重），但
-- pack apply / 内容包导入走的是「按名 NOT EXISTS 补种」裸 SQL——
-- 0442 已经证明这套口径会出事：0422 按名补种第二代词表，与 0175 的
-- 第一代名字不同（H.264 vs H.264/x264），两代共存导致发布页下拉
-- 出现重复选项，最后靠 0442 手工引用映射 + 删 gen1 收场。
--
-- 病根当时没除：只要「包内名字 ≠ 库里已有名」的窗口存在（改名、
-- 大小写差异、前后空格），NOT EXISTS 补种就会再长出并行词条。本迁移
-- 把管理端的查重口径（lower(trim(name))）下沉为 DB 约束，从根上
-- 断掉复发路径：
--
--   1) 存量体检：同维度 normalize 后重复的行先合并——保留 id 最小的
--      （更早、更可能被 torrent_sections 引用），把其它重复行上的
--      种子归属改挂到保留行，再删重复行。合并必须先于建索引，否则
--      脏数据会让 CREATE UNIQUE INDEX 直接失败（当前各环境体检为
--      0 行重复，此段是防御性的幂等逻辑）。
--   2) 唯一表达式索引 + NULLIF 口径：name 为空串的行不参与唯一
--      （与 categories key 的表达式索引同款手法，0317）。空串词条
--      本就是非法输入（管理端 name.trim().is_empty() 拒收），但历史
--      库不做假设。
--   3) 撞唯一索引的 INSERT（并发补种/手工 SQL）会得到 23505 错误，
--      这正是期望的行为——调用方应改走 ON CONFLICT 或先查后写。
--      pack_core 的 NOT EXISTS 补种在同一事务内串行执行，索引建立
--      后该分支自然不再产生重复（NOT EXISTS 命中即跳过）。
--
-- 幂等：IF NOT EXISTS + 防御性合并段重跑安全（无重复时 UPDATE/DELETE
-- 均为零行）。

-- ① 存量合并：同维度 normalize 同名的重复行，归属改挂 id 最小行
--    （torrent_sections.dict_id 是 ON DELETE CASCADE，不先改挂直接删
--    会静默抹掉种子归属——这正是 0442 手工做的事，这里固化成迁移）
UPDATE torrent_sections ts
SET dict_id = keep.id
FROM section_dict dup
JOIN section_dict keep
  ON keep.kind = dup.kind
 AND lower(trim(keep.name)) = lower(trim(dup.name))
 AND keep.id < dup.id
WHERE ts.dict_id = dup.id
  AND NOT EXISTS (
    SELECT 1 FROM torrent_sections x
    WHERE x.torrent_id = ts.torrent_id
      AND x.kind = ts.kind
      AND x.ordinal = ts.ordinal
      AND x.dict_id = keep.id)
  AND dup.name <> '';

-- ② 删除合并后的空挂重复行（已无 torrent_sections 引用；pack 层引用
--    无 FK，词表重建时按名对齐，删行无悬空风险）
DELETE FROM section_dict dup
USING section_dict keep
WHERE keep.kind = dup.kind
  AND lower(trim(keep.name)) = lower(trim(dup.name))
  AND keep.id < dup.id
  AND dup.name <> '';

-- ③ 唯一约束落地（lower(trim()) 与管理端查重、0442 收敛口径一致）
CREATE UNIQUE INDEX IF NOT EXISTS uq_section_dict_kind_name
  ON section_dict (kind, lower(trim(name)))
  WHERE name <> '';
