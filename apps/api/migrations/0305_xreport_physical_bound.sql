-- 0305（tracker 五轮深挖，2026-10-08，报告 _doc/PT-tracker五轮深挖实测审计-2026-10-08.md）
-- 交叉佐证必须受物理上界约束。
--
-- 现状：`upload_corroborated.bytes` 是 leecher 自报量的无界累加（单笔只卡
-- 「>10 GiB 判 absurd」），而 process_event 正是拿它当「自报上传量还能入账
-- 多少」的上界；2026-10-08 的辅种豁免更把判据写成 corroborated > 0。
-- 两条合起来 = 两个账号互相 xreport 即可无限铸上传额，且 1 字节就能把
-- 幽灵做种/完成数两条不变量一起作废（探针 D 组实测：B 从未下载过任何字节，
-- 一条 xreport=<A 的 peer_id>:1 之后 A 的 seeding 直接翻 true）。
--
-- 不变式：一个 leecher 在这颗种上能背书的总量 ≤ 它自己被记账的 credited 下载
-- （它只能为「自己真的收到过的字节」作证）。
--
-- 为什么要**新表**而不是聚合 leecher_xreports：明细表按 7 天就地裁剪
-- （0301 的口径，供 down_under 对账用），拿被裁剪的表算上界等于每 7 天
-- 免费重置一次额度。认可量必须长存，所以单独一张按 (leecher, info_hash)
-- 单调累加的台账；leecher_xreports.bytes 保持「原始自报」口径不动，
-- 对账链不受影响。
--
-- 全部幂等（IF NOT EXISTS），重跑安全。

CREATE TABLE IF NOT EXISTS xreport_credited (
    leecher    bigint       NOT NULL,
    info_hash  text         NOT NULL,
    credited   bigint       NOT NULL DEFAULT 0,
    updated_at timestamptz  NOT NULL DEFAULT now(),
    PRIMARY KEY (leecher, info_hash)
);
COMMENT ON TABLE xreport_credited IS
'交叉佐证的已认可总量（按 leecher × 种子单调累加）：佐证上界的账本。'
'leecher_xreports 是 7 天滚动的自报明细（对账用），本表不清理——'
'否则额度会随明细裁剪而重置。';

-- 认可量与自报量并存，便于「谁在超报」的追查直接按行比对
ALTER TABLE leecher_xreports
    ADD COLUMN IF NOT EXISTS over_bytes bigint NOT NULL DEFAULT 0;
COMMENT ON COLUMN leecher_xreports.over_bytes IS
'本条自报佐证量中**未被认可**的部分（超出佐证者自己 credited 下载量的余额）。'
'bytes 仍是原始自报口径，down_under 对账不变。';
