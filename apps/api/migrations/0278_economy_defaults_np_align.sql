-- 0278：经济系统默认值对齐 NexusPHP 官方口径
--
-- 背景：等级 0277 对齐后，经济默认同样向 NP 官方出厂看齐（出处：
-- xiaomlove/nexusphp settings.default.php L208-248 + BonusLogs.php 常量 +
-- Attendance.php 常量，v1.8.0 tag 交叉验证一致）。45 项对位后 17 项有差，
-- 本迁移只改「有差且 NP 语义更合理」的 13 项；本站机制性差异保留：
--   · noad_hours（NP 免广告是商店 15 天装；本站是后台时长制，不动）
--   · attendance_first=15（本站首签高于 NP 常量 10，运营口径已有意调高）
--   · basictax/taxpercentage 礼物税（本站礼物系统未启用税收，保持 0）
--   · prolinkpoint 推广点击（本站无推广链路，保持 0）
-- 已运营站点后台可改；商店价目改的是「出厂默认」。

BEGIN;

-- ===== 商店价目（NP 官方默认） =====
UPDATE site_settings SET value = '1300',   updated_at = now() WHERE name = 'tengbupload';          -- 10GB 上传 1200→1300
UPDATE site_settings SET value = '1000',   updated_at = now() WHERE name = 'oneinvite';           -- 邀请 10000→1000
UPDATE site_settings SET value = '500',    updated_at = now() WHERE name = 'one_tmp_invite';      -- 临时邀请 5000→500
UPDATE site_settings SET value = '100000', updated_at = now() WHERE name = 'change_username_card'; -- 改名卡 10000→100000
UPDATE site_settings SET value = '5000',   updated_at = now() WHERE name = 'rainbow_id';          -- 彩虹 ID 10000→5000
UPDATE site_settings SET value = '10000',  updated_at = now() WHERE name = 'cancel_hr';           -- 消 H&R 20000→10000
UPDATE site_settings SET value = '1000',   updated_at = now() WHERE name = 'attendance_card';     -- 补签卡 5000→1000

-- ===== 魔力公式系数 =====
UPDATE site_settings SET value = '0.5',    updated_at = now() WHERE name = 'official_addition';   -- 官方种加成 0→0.5（NP 默认）

-- ===== 购买上传量资格闸门（NP：分享率>6 或上传>50GB 者禁买——防大号刷） =====
-- 本站原 0.5 太松（几乎所有用户都被拦在另一头：稍高分享率即禁买反常），
-- 对齐 NP 语义：只有「已经很有钱」的用户才禁买。
UPDATE site_settings SET value = '6',      updated_at = now() WHERE name = 'ratiolimit';          -- 0.5→6
-- dlamountlimit 50GB 与 NP 一致，不动

COMMIT;
