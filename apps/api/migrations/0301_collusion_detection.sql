-- 0301（假种/作弊审计缺口三件套，2026-10-07，报告 _doc/假种与作弊漏洞深度审计-2026-10-07.md）
-- 三类只在「数据交叉」后才现形的作弊：
--   ① 同 IP 双账号对刷（A 做种 B 下载账面全真、实为一人对倒）
--   ② 谎报下载量（xreport 佐证和 >> 自报 downloaded）
--   ③ BitThief（完成下载后零上传长期挂机）
-- 数据基础：announce 事件流带 ip/user/hash 但不落表，对刷检测需要窗口内
-- 的 (ip, user, hash) 三元组可聚合 → 轻量日志表；xreport 佐证按 leecher
-- 维度的汇总需要事件表（worker 消费时落账，供 down_under 对账）。
-- 全部幂等（IF NOT EXISTS），重跑安全。

-- ① announce 来源日志（对刷检测的窗口聚合源；worker 就地裁剪 2h 前数据）
CREATE TABLE IF NOT EXISTS announce_ips (
    id         bigserial PRIMARY KEY,
    user_id    bigint NOT NULL,
    ip         text NOT NULL,
    info_hash  text NOT NULL,
    seeding    boolean NOT NULL DEFAULT false,
    leeching   boolean NOT NULL DEFAULT false,
    seen_at    timestamptz NOT NULL DEFAULT now()
);
CREATE INDEX IF NOT EXISTS idx_announce_ips_window
    ON announce_ips (seen_at, ip, info_hash);
COMMENT ON TABLE announce_ips IS
'announce 来源三元组日志（user/ip/hash+角色）：同 IP 双账号对刷检测的窗口聚合源。只保留 2 小时窗口，由 worker 消费循环就地裁剪。';

-- ② leecher 佐证明细（down_under 对账源；同样短窗口，按 leecher 汇总）
CREATE TABLE IF NOT EXISTS leecher_xreports (
    id           bigserial PRIMARY KEY,
    leecher      bigint NOT NULL,
    uploader     bigint NOT NULL,
    info_hash    text NOT NULL,
    bytes        bigint NOT NULL,
    reported_at  timestamptz NOT NULL DEFAULT now()
);
CREATE INDEX IF NOT EXISTS idx_leecher_xreports_window
    ON leecher_xreports (reported_at, leecher);
COMMENT ON TABLE leecher_xreports IS
'leecher 交叉上报明细（佐证「某 leecher 收到了 N 字节」）：谎报下载量对账源——佐证和显著大于自报 downloaded 即命中。保留 7 天窗口。';
