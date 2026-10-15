-- 0376 版块配置乐观并发（FRAME §5 推广项之二：版主配置）
--
-- 背景：`PUT /admin/forums/{id}` 此前是 last-write-wins——两名版主同时
-- 打开同一版块的设置页，A 改标题、B 改权限门槛，两人都点保存，后到的
-- 那次把前一次整条覆盖且**毫无感知**。三档门槛（minclassread /
-- minclasswrite / minclasscreate）被静默清成 0/0/0 就是这类覆盖的
-- 典型后果：权限一次性放宽到所有人，而日志里看不出是谁在什么时候
-- 抹掉的。
--
-- 口径（与 torrent_batch 的 hr_policy 写入同模式，0353 那套）：
--   · forums 加 mtime 列作为版本戳，默认 now()，已有行一次性对齐；
--   · 编辑表单携带 expected_mtime 时，UPDATE 带上
--     `mtime = $expected::timestamptz` 的 WHERE 条件，命中 0 行即 409；
--   · 携带即校验，缺省不校验（灰度期不改前端也能上）。
--
-- 为什么是 mtime 而不是独立 version 计数器：forums 行的写入方只有
-- 这一处，mtime 已经是事实上的版本戳；再加一列 version 只会多一处
-- 要记得 +1 的地方。torrents 那边用 mtime 同理。
--
-- 409 走既有的 DomainError::VersionConflict，不新增错误码。

ALTER TABLE forums
  ADD COLUMN IF NOT EXISTS mtime timestamptz NOT NULL DEFAULT now();

-- 已有行统一对齐到 now()：DEFAULT 对存量行同样生效（PG11+ 的
-- ADD COLUMN ... DEFAULT 会重写表并回填），此句只为在重复执行时兜底。
UPDATE forums SET mtime = now() WHERE mtime IS NULL;
