-- 0303 待审种子的 tracker 准入策略（审计 2026-10-07 P1-2）
-- 依据：_doc/PT-tracker深挖实测审计报告-2026-10-07.md §四 P1-2。
-- 站点侧 visibility.rs 把 approval_status=0（待审）的种子对非发布者/非员工隐藏
-- （详情 404），而 tracker 白名单放行集是 approval_status IN (0,1) 且对**任意**
-- passkey 用户开放 swarm —— 实测：待审种子的 announce 会返回发布者真实 ip:port，
-- scrape 也给实时做种数。审核中/暂缓公开的内容从数据面外泄。
-- 默认 self_seed_only：照常接受 announce（发布者审核期要能做种、事件照常计费），
-- 但对非发布者/非员工清空 peer 列表与计数；站长可切 allow_all（旧行为）或
-- owner_only（直接拒）。策略必须可配——本项目是建站系统，不把某一类站的审核
-- 流程焊进共享层。

INSERT INTO site_settings (name, value, descr, grp)
VALUES ('announce_pending_policy', 'self_seed_only',
        '待审种子的 tracker 准入：allow_all | self_seed_only | owner_only',
        'anticheat')
ON CONFLICT (name) DO NOTHING;

INSERT INTO settings_meta (name, type, label_zh, label_en, hint, options,
                           group_key, card_order, min_class, visible)
VALUES ('announce_pending_policy', 'enum',
        '待审种子准入', 'Pending torrent access',
        '审核中的种子是否允许非发布者进入 swarm。self_seed_only：可 announce 但不外发他人 peer 与计数。',
        '[{"l":"全部放行（旧行为）","v":"allow_all"},{"l":"可做种但不外发他人（默认）","v":"self_seed_only"},{"l":"仅发布者与员工","v":"owner_only"}]'::jsonb,
        'anticheat', 13, 99, TRUE)
ON CONFLICT (name) DO NOTHING;
