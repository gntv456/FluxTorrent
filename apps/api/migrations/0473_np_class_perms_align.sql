-- 0473: 各职级权限对齐 NexusPHP 官方口径（2026-11-07）。
--
-- 权威来源（仓库内 .np-research/np，php8 分支）：
--   include/core.php:37-53          —— UC_* 17 档常量
--   nexus/Install/settings.default.php:134-190 —— authority 默认最低档
--   public/staffpanel.php:21/42/63  —— 面板三分组 15/14/13
--   public/*.php                    —— 各页 get_user_class() 硬门槛
--
-- 档位映射（NP → 本站 user_classes）：
--   UC_VIP 10            → 20 贵宾
--   UC_RETIREE 11        → 本站原缺，本次补 21 养老组
--   UC_UPLOADER 12       → 91 发布员
--   （版主）NP 无独立档  → 92 论坛版主 + role/forum_mod 职务承接
--   UC_MODERATOR 13      → 93 总版主
--   UC_ADMINISTRATOR 14  → 94 管理员
--   UC_SYSOP 15          → 98 维护开发员（0472 已对齐：settings.php
--                          权限矩阵 / massmail / mysql_stats / maxlogin
--                          / location / increment-bulk 全是 15 档）
--   UC_STAFFLEADER 16    → 99 站长
--
-- 现状差异（重建 role_permissions 累积态后逐键比对）：
--   · 本站把 28 把内容/监察键一股脑挂在 class/90「幽灵锚点」上，
--     NP 里这些键分属 13（funmanage/postmanage/torrentmanage/clearcache
--     /stats/allagents/admanage/ipcheck）与 14（newsmanage/linkmanage/
--     faqmanage/pollmanage/forummanage）。本站更宽。本次**不收窄**：
--     收窄会让 91/92 现有职员整片失去后台入口，后台导航分组、
--     探针脚本与既有运营都建在这条线上，属产品决策，留待站长定。
--   · 反过来有一批「NP 13/14 就有、本站却只授到 95/98/99」的键，
--     这才是中档看不到功能的真因。本次补齐（第 2 节）。
--   · NP 12 发布员的 viewanonymous / uploadspecial / seebanned /
--     torrent-approval-allow-automatic / torrent-set-price 在本站挂在
--     role/uploader 职务上，等级 91 本身没有；补成 NP 的等级直授。
--
-- 幂等：档位与授权都带 ON CONFLICT；防呆只读，重放无副作用。

BEGIN;

-- ============ 1) 养老组档位（NP UC_RETIREE = 11） ============
-- NP 里养老族是荣誉身份档，几乎无专属代码特权，但数值 >= VIP(10)，
-- 因而继承 VIP 的免 H&R / 免等待 / 账号不清理。本站 class 授权是累进
-- 语义（0056：class_id >= role_key），21 >= 20 自动继承
-- hr.exempt / invites.bonus / seed.stats.view，无需重复写授权行。
-- 不建 class_rules 规则行：worker 的升降只看 0-12 的成长档（0277）。
INSERT INTO user_classes
    (id, name, pod_level, min_uploaded, min_ratio, min_age_days,
     privileges)
VALUES (21, '养老组', 21, 0, 0, 0, '{}'::jsonb)
ON CONFLICT (id) DO UPDATE SET name = EXCLUDED.name;

-- ============ 2) 三档补权：NP authority 默认值 → 本站权限键 ============
-- 91 ← UC_UPLOADER(12)：免审种、改价、特殊种子。
--   torrent.view_anonymous / torrent.see_banned 已由 class/90 覆盖，
--   不重复授；set_price / upload_special 当前 implemented=false
--   （0360/0062），先按 NP 口径把档位钉好，接线后自动生效。
-- 93 ← UC_MODERATOR(13)：cheaters.php 在 NP 是 13 档，本站 0361
--   为「名单含上传增量」收窄到 95；补 93 仍把 91/92 挡在外面。
-- 94 ← UC_ADMINISTRATOR(14)：adduser.php / reset.php /
--   user-change-class / delacctadmin(user-delete) / catmanage.php /
--   fields.php 在 NP 全是 14 档，本站此前只在 98/99。
--   改档口的「封顶 = 操作者等级 - 1」护栏已在 user_status.rs:272
--   与 users_batch.rs:49，管理员提不了 98/99，无提权链。
--   ⚠️ freeleech.php 在 NP 也是 14，但本站「免费/促销状态」总面板
--   经审计修复统一收在 settings.manage（0420 把导航钉同键），
--   属 NP 15 口径的刻意收窄；这里不下放 freeleech.manage，
--   否则 94 拿到键却仍被导航挡住，又成「授权了没入口」。
--   NP 15 档专属键（settings/massmail/运维/批量发放）一律不下放。
INSERT INTO role_permissions (role_type, role_key, permission_key, granted)
SELECT 'class', v.role_key, p.key, true
  FROM (VALUES
    ('91'::text, 'torrent.approval.auto'::text),
    ('91',      'torrent.set_price'),
    ('91',      'torrent.upload_special'),
    ('93',      'cheaters.view'),
    ('94',      'user.create'),
    ('94',      'user.resetpass'),
    ('94',      'user.class'),
    ('94',      'user.delete_disabled'),
    ('94',      'categories.manage'),
    ('94',      'userfields.manage')
  ) AS v(role_key, perm_key)
  JOIN permissions p ON p.key = v.perm_key
ON CONFLICT (role_type, role_key, permission_key)
DO UPDATE SET granted = true;

-- ============ 3) 防呆 ============
DO $$
DECLARE
    n int;
BEGIN
    SELECT count(*) INTO n FROM user_classes
     WHERE id = 21 AND name = '养老组';
    IF n <> 1 THEN
        RAISE EXCEPTION '养老组档位(21) 未就位';
    END IF;

    -- 10 条 NP 对齐授权必须在册（JOIN permissions 保证不写错键）
    SELECT count(*) INTO n FROM role_permissions
     WHERE role_type = 'class' AND granted AND (
       (role_key = '91' AND permission_key IN (
         'torrent.approval.auto', 'torrent.set_price',
         'torrent.upload_special'))
       OR (role_key = '93' AND permission_key = 'cheaters.view')
       OR (role_key = '94' AND permission_key IN (
         'user.create', 'user.resetpass', 'user.class',
         'user.delete_disabled', 'categories.manage',
         'userfields.manage')));
    IF n <> 10 THEN
        RAISE EXCEPTION 'NP 对齐授权预期 10 条，实际 % 条', n;
    END IF;

    -- 诊断（只告警不阻断）：线上权限矩阵允许站长手工改，历史行里若
    -- 有人给中档授过 15 档专属键，不该让本迁移整体失败。本次 VALUES
    -- 的 10 条已静态核对，不含任何 15 档专属键。
    SELECT count(*) INTO n FROM role_permissions
     WHERE role_type = 'class' AND role_key IN ('91', '93', '94')
       AND granted
       AND permission_key IN (
         'settings.manage', 'massmail', 'emailban.manage',
         'syslog.view', 'dbstats.view', 'cleanup.run',
         'maxlogin.view', 'locations.manage', 'tracker.manage',
         'user.amountbonus', 'user.amountupload', 'roles.manage',
         'layout.manage', 'custompages.manage', 'terms.manage',
         'wiki.manage', 'user.passkey.reveal');
    IF n > 0 THEN
        RAISE NOTICE '% 条 NP 15 档专属键在中档，请人工核对', n;
    END IF;
END $$;

COMMIT;
