-- 0347_attach_sha_user_dedup.sql
-- P-57 存量修复：附件表的 sha256 唯一索引与「按人隔离权限行」互斥。
--
-- 0100 建表时按「同文件全站只存一份」的图床语义下了
--   CREATE UNIQUE INDEX attachments_sha_key ON attachments (sha256);
-- 0294（安全审计 P1-2）把可见性做进 attachments.visibility，并把去重判据
-- 改成「同人已有行才复用；他人已有行且本次要 private 时插独立行，物理文件
-- 仍按 sha 共享」。这条新语义要求同一个 sha 能有多行，而那条唯一索引从没
-- 被 DROP 过（live `\d attachments` 复核：attachments_sha_key UNIQUE btree
-- (sha256) 仍在）⇒「插独立行」这条路永远撞约束。
--
-- 实测（两个不同账号，同一份字节，先后两种顺序）：
--   成员 shared → 200；站长同字节 private → **500**，库里该 sha 仍只有 1 行
--   成员 private → 200；站长同字节 shared → **500**，库里仍 1 行
-- 即两个人传同一张截图/同一张封面，只要有一方要走私有，后来那位直接失败
-- （图床最常见的形态）。读取端 visibility_denied 也已按「他人行」写，
-- 本就假设多行存在。
--
-- 换成 (sha256, user_id) 唯一：同一人重复上传仍然去重（响应里的
-- deduplicated 语义不变），不同人各自一行，权限按人隔离真正成立。
-- 左列 sha256 保留索引能力，读取端 WHERE sha256 = $1 不受影响。
DROP INDEX IF EXISTS attachments_sha_key;
CREATE UNIQUE INDEX IF NOT EXISTS attachments_sha_user_key
    ON attachments (sha256, user_id);

-- P-59 接线：enableattach 自 0034 起就在 settings_meta 里（后台
-- 「附件设定」可见可改），但全仓零消费者——它是 fake_switch 基线 289 条里
-- 的一条。实测现值 no 时上传仍 200：站长以为关掉了附件功能，实际没关。
-- 代码侧现在读它（两个上传口都判）。为不改变既有站点行为，把种子值从
-- 与 NP 转写残留的 'no' 归位到 'yes'（今天的行为），此后关掉才真会拒。
UPDATE site_settings
   SET value = 'yes'
 WHERE name = 'enableattach'
   AND value = 'no';
