-- 0323_torrent_artifacts.sql
-- 站型成熟度 · 阶段 2 game/software 批（对标 §7.4「版本号+校验和+更新日志」P0）。
--
-- 游戏/软件站的两个最基础问题：「这版是不是最新」「包体坏没坏」。
-- 对标 GGn：GameDOX（更新包）挂到本体条目 + 校验和可核对。
--
-- 承载评估（与 movie 批同思路，优先维度系统）：
--   版本号 = version 维度（0315 已配 text）——可写可筛 ✅
--   更新说明 = 包内描述/评论——已有
--   **校验和 + 更新链** = 无承载 ⇒ 本迁移补 torrent_artifacts 工件表：
--     一行 = 一个种子的一件工件（校验和清单/更新日志/许可文件…），
--     kind 区分工件类型，sha256 服务端可复算核对，parent_id 把
--     「升级补丁/更新包」挂到本体种子（GGn GameDOX 语义的最小实现）。
--
-- 消费面（后续批次接 UI/审核台）：详情聚合 artifacts 段 + 更新链导航。
-- 本批先落表 + 发种侧写入通道（multipart artifact part，同 log part 范式）
-- + 详情读口，UI 徽标随后。

BEGIN;

CREATE TABLE IF NOT EXISTS torrent_artifacts (
    id BIGSERIAL PRIMARY KEY,
    torrent_id BIGINT NOT NULL REFERENCES torrents(id) ON DELETE CASCADE,
    -- 本体种子（更新包场景）：NULL = 自身即本体
    parent_torrent_id BIGINT REFERENCES torrents(id) ON DELETE SET NULL,
    -- 工件类型：checksums（校验和清单）/ changelog / license / other
    kind TEXT NOT NULL DEFAULT 'other',
    filename TEXT NOT NULL DEFAULT '',
    -- 工件内容（文本类：SFV/MD5 清单、changelog 正文）
    body TEXT NOT NULL DEFAULT '',
    -- 整包 sha256（上传时客户端算好；服务端复算核对留给审核台批次）
    sha256 TEXT,
    size_bytes BIGINT NOT NULL DEFAULT 0,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    UNIQUE (torrent_id, kind, filename)
);

CREATE INDEX IF NOT EXISTS idx_torrent_artifacts_parent
    ON torrent_artifacts (parent_torrent_id);

COMMENT ON TABLE torrent_artifacts IS
    '种子工件（0323 game/software 批）：校验和清单/更新日志等附件级元数据，'
    'parent_torrent_id 承载更新包→本体的挂链（GGn GameDOX 语义最小实现）';

COMMIT;
