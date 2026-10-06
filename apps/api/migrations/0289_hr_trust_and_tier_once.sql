-- 0289：H&R 处置可信 + 捐赠档位不重发（2026-10-06 版主第三轮实测）
--
-- 三条实测根因：
--   P0  `hr.rs` 每轮把「违规数低于阈值」的账号 download_enabled 一律置 TRUE，
--       而版主手动冻结作弊嫌疑账号写的也是同一列 ⇒ 冻结 5 分钟内自动失效
--       （job 5min 一跳）。代码注释自己写了「不区分当初被禁原因」。
--       修法：给下载开关记「谁锁的」，H&R 只解自己锁的。
--   P0  捐赠档位发放的 CAS 键是 `payment_orders.tier_granted`（按单），
--       而档位取的是**累计**实付的最高档 ⇒ 跨过 $10 之后每一单都再发一次
--       同一档奖励（魔力/上传量/邀请/勋章），小额多次捐赠＝无限复领。
--       修法：新增按 (user, 档位门槛) 唯一的一经发放台账。
--   P1  `hr_violations` 只有人工赦免才写 resolved_at，而违规通知与封禁通知
--       都向会员承诺「持续做种达标后自动消除」⇒ 承诺为假，3 次违规的旧账
--       永远算在头上。修法：达标即自清并回签一条消除说明。
--
-- ⚠️ 全部幂等；值行先于登记行（settings_meta.name 有 FK → site_settings.name）。

ALTER TABLE users ADD COLUMN IF NOT EXISTS download_locked_by text;
COMMENT ON COLUMN users.download_locked_by IS
    'download_enabled=false 的下锁方：staff=管理组手动（H&R 不得自动解）/ hr=H&R 自动 / NULL=历史数据';

-- 历史数据里「已被禁用但没有锁来源」的行按 staff 处理：宁可让管理组手动复核，
-- 也不能让一个作业悄悄放开别人手里的冻结。
UPDATE users SET download_locked_by = 'staff'
 WHERE NOT download_enabled AND download_locked_by IS NULL AND status < 2;

CREATE TABLE IF NOT EXISTS user_donation_tiers (
    user_id   bigint NOT NULL,
    min_usd   numeric(10,2) NOT NULL,
    tier_name text,
    order_no  text,
    granted_at timestamptz NOT NULL DEFAULT now(),
    PRIMARY KEY (user_id, min_usd)
);
COMMENT ON TABLE user_donation_tiers IS
    '捐赠档位一经发放台账（0286）：档位按累计实付判定，但只能按 (人, 档) 发一次';

-- H&R 总开关真生效（此前 enable_hr 是后台能改、代码零读的假开关）
INSERT INTO site_settings (name, value, descr, grp)
VALUES ('enable_hr', 'yes',
        '启用 H&R（下载后必须保种）判定与处置：no = 不建快照、不判违规、不自动停下载',
        'anticheat')
ON CONFLICT (name) DO NOTHING;

INSERT INTO settings_meta (name, type, label_zh, label_en, hint, group_key, card_order, visible)
VALUES ('enable_hr', 'bool', '启用 H&R 判定', 'Enable hit-and-run',
        '关闭后 worker 不再为新完成下载建快照，也不再判违规/停下载；'
        '已存在的违规记录与人工处置不受影响。',
        'anticheat', 14, true)
ON CONFLICT (name) DO NOTHING;
