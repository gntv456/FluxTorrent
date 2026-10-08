-- 0310（通用 PT 站点适配，2026-10-08）connectable 第四档 SUSPECT。
--
-- 背景：0308 已按主流站口径把 connectable_gate 缺省改为 off（UNIT3D
-- `connectable_check` 默认 false、Gazelle `xbt_files_users.connectable`
-- 默认 1 且只当信息位），即「回连不可达」不再一票否决在种。这解决的是
-- **要不要罚**；本迁移解决的是 **能不能区分**。
--
-- 问题：BT 三阶段探测（握手 → bitfield → piece SHA-1）此前把两类性质
-- 完全不同的 peer 压成同一个值 0（不可信）：
--   · 裸监听占位 / 端口不通        —— 实锤作弊，应罚
--   · 不响应明文 BT 协议的用户      —— 大概率是**加密客户端**
--     （qBittorrent「仅加密连接」、MSE-PE、peer 白名单）或被 CGNAT/
--     防火墙挡住 —— **好用户，不该罚**
-- 私有站客户端基线已知、可以要求用户关加密；**通用站必须假设用户群里
-- 有各种客户端偏好**，两者同判就误伤站内做种供给。而一旦合并成同一个
-- 值，事后**再也分不出**这两种人，事后补救无从下手。
--
-- 方案：引入第四档 -2 = SUSPECT（无法验证）。语义与消费口径：
--   · -1 UNTESTED —— 本轮未探测，不作判定
--   · -2 SUSPECT —— 端口通但不响应明文 BT 协议，无法验证
--   ·  0 DEAD    —— 实锤不可信（端口不通 / piece 哈希不符伪造数据）
--   ·  1 OK      —— 实测可信
--
-- **口径不变的部分（重要）**：只有 0 阻断收益/惩罚，-2 与 -1 同样放行。
-- 也就是说本迁移**不改变任何用户的收益**，只把「无法验证」从「不可信」
-- 里分离出来、让它可观测、可统计、可据此调策略。这与 0308 的
-- connectable_gate=off 是同一方向：宁可放过、不可误伤。
--
-- 列类型无需变更：smallint 天然容纳 -2（且无 CHECK 约束）。本迁移只补
-- 取值域约束与注释，并给管理端一个观测入口所需的索引。

-- ① 取值域注释（把第四档写进 schema 契约，避免后人误以为只有三态）
COMMENT ON COLUMN snatches.connectable IS
'BT 探测结论（tracker 侧三阶段：握手 → bitfield → piece SHA-1）：
 -1 UNTESTED = 本轮未探测，不作判定；
 -2 SUSPECT = 端口可连但不响应明文 BT 协议，**无法验证**（典型：仅加密
      连接客户端 / MSE-PE / peer 白名单 / CGNAT），不惩罚、仅观测；
  0 DEAD    = 实锤不可信（端口不通，或 piece SHA-1 与 info.pieces 不符
      即伪造数据），阻断做种收益与保种结算；
  1 OK      = 实测可信。
消费口径：仅 0 阻断；-2 与 -1 放行。0308 起 connectable_gate 缺省 off，
即使 0 也不否决在种（结果只留痕供面板筛），可用该设置切到 hard。';

-- ② 管理端观测入口索引：按结论分组统计「无法验证」占比。
--    通用站运营需要这个数来判断「加密客户端占比」vs「真实作弊占比」，
--    进而决定是否收紧口径。partial index 只覆盖需要聚合的取值。
CREATE INDEX IF NOT EXISTS idx_snatches_conn_state
    ON snatches (connectable)
    WHERE connectable IS NOT NULL;

-- ③ 观测口径说明（供管理端与后续实现对齐，避免各写各的）。
--    期望查询口径：
--      可疑占比 = count(*) FILTER (WHERE connectable = -2) / count(*)
--      不可信数 = count(*) FILTER (WHERE connectable = 0)
--    按 agent（客户端 UA）分组可初步区分「加密客户端」与「伪造客户端」。
COMMENT ON TABLE snatches IS
'做种/下载状态表。connectable 列的 -2(SUSPECT) 档专为通用 PT 站点的
「无法验证但不该罚」场景而设，详见该列 COMMENT。';