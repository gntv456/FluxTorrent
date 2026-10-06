-- 0294 附件可见性（2026-10-06 全站安全审计 P1-2）
--
-- 背景：GET /attachments/{sha} 此前只验「登录 + sha 形状」，无属主/可见性
-- 判定——任何成员拿到 sha（URL 会嵌进帖子/简介，帖子编辑删除后仍残留于
-- 历史/日志/转发）即可永久读取任意附件，且全站 sha 去重使同内容只有一行、
-- 无法按人撤销。
--
-- 方案：可见性三态 + 引用授权。
--   visibility ∈ ('private','shared','staff')，缺省 'shared'：
--   - shared  ：公开引用语义（现状）——种子里梗/论坛帖/公告里引用的图与
--               字幕包都靠它；任何成员可读。存量数据保持 shared 不动，
--               避免升级即把全站已引用附件锁死（帖子里的图全部 404）。
--   - private ：仅上传者本人与 staff（class_id >= 90）可读。
--   - staff   ：仅 staff 可读（预留给管理组内部材料）。
--   上传端默认 shared（图床语义），私有上传传 visibility=private。
--   读取端（GET/HEAD）按 visibility + 属主/staff 判定；dedup 命中时
--   「同人已有行」沿用自己那行的可见性（唯一索引仍按 sha 全站一行，
--   可见性是行级属性——后续要按人撤销走管理端下架）。
--   管理端按 sha 的属主查询/改可见性入口见 admin 侧（本迁移只落数据面）。
ALTER TABLE attachments
    ADD COLUMN IF NOT EXISTS visibility text NOT NULL DEFAULT 'shared'
    CHECK (visibility IN ('private','shared','staff'));

-- 索引：按可见性巡检（管理端「私有附件清单」）
CREATE INDEX IF NOT EXISTS attachments_visibility_idx
    ON attachments (visibility, created_at DESC)
    WHERE visibility <> 'shared';
