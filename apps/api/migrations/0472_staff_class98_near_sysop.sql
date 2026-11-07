-- 0472: 维护开发员（class 98）对齐站长档——「接近站长」（2026-11-07）。
--
-- 现象：管理组面板对 98 档只露出约一半功能。两道门叠着：
--   1) staffpanel 读口 WHERE min_class <= class_id（admin_http/site.rs:39），
--      0052/0055/0063/0209/0226/0421 共 19 条钉死 min_class=99；
--   2) perm_key 非空还要 user_can()（site.rs:47-58），0420 给 12 条
--      90/93 档入口钉上 99 专属键，等于把入口又关了一遍。
-- 决定：98 档定位「接近站长」——凡 99 档凭 class 拿到的权限，98 全有；
--   上述 19 条导航随之下放到 98，12 条被键挡住的入口自然放行。
-- 保留的护城河（不动，仍只有 99）：不能把任何人设为 99
--   （user_status.rs:269）、不能改 99 账号（:282 / ensure_outranks）、
--   批量改档值域封顶 98（users_batch.rs:42）。另有两个原本只 99
--   才会碰到的口子，随键下放后同步补了「目标等级 < 操作者等级」：
--   重置密码 / 批量删被禁用户（staff_http/ipcheck.rs），否则 98 能
--   重置站长密码或删掉被封的 99 账号。即 98 能管站，但造不出、
--   也动不了站长本人。
-- 另：站点设定字段写入是 Rust 常量（settings_http/meta.rs 的
--   SETTINGS_WRITE_MIN），不在 role_permissions 里；本迁移只管数据，
--   常量随代码同步改成 98，否则设置页仍回 editable=false / 403。
--
-- 幂等：授权 ON CONFLICT DO UPDATE，导航 UPDATE 带目标状态限定，
--   补键只动 NULL；只读防呆，重放无副作用。

BEGIN;

-- ============ 1) 99 档的 class 权限全量镜像给 98 ============
-- 执行时 class/99 实有 26 把：0054 的 17 + 0063 tracker.manage
-- + 0064 roles.manage + 0080 torrent.see_banned + 0217 三把细键
-- + 0285 user.passkey.reveal + 0359 wiki.manage + 0471 layout.manage。
-- 用动态复制而非手写清单，免得漏键；user_can() 是累进判定，
-- 授给 98 后 99 仍自动持有。
INSERT INTO role_permissions (role_type, role_key, permission_key, granted)
SELECT 'class', '98', rp.permission_key, true
  FROM role_permissions rp
 WHERE rp.role_type = 'class' AND rp.role_key = '99' AND rp.granted
ON CONFLICT (role_type, role_key, permission_key)
DO UPDATE SET granted = true;

-- ============ 2) 19 条 99 专属导航下放到 98 ============
-- 只按白名单改；站长手工删掉的条目不补、手工调过的其它条目不动。
UPDATE staff_panel_entries SET min_class = 98
 WHERE min_class = 99
   AND tab_key IN (
     'deldisabled', 'forums', 'settings', 'mail', 'emailbans',
     'syslog', 'dbstats', 'cleanup', 'bans', 'maxlogin',
     'agentrules', 'locations', 'plugins', 'perm', 'trackers',
     'panelentries', 'onboarding', 'economy', 'ops');

-- ============ 3) 补 perm_key：外露必须打得开（0420 轴） ============
-- 这 14 条历史 perm_key 为 NULL，只靠 min_class 放行；补成后端首屏
-- 读口那把键，站长日后在权限矩阵撤键时导航会自动跟着收起。
UPDATE staff_panel_entries e
   SET perm_key = v.perm_key
  FROM (VALUES
    ('deldisabled'::text, 'user.delete_disabled'::text),
    ('mail',            'massmail'),
    ('emailbans',       'emailban.manage'),
    ('syslog',          'syslog.view'),
    ('dbstats',         'dbstats.view'),
    ('cleanup',         'cleanup.run'),
    ('maxlogin',        'maxlogin.view'),
    ('locations',       'locations.manage'),
    ('plugins',         'plugins.manage'),
    ('agentrules',      'settings.manage'),
    ('perm',            'settings.manage'),
    ('trackers',        'tracker.manage'),
    ('onboarding',      'settings.manage'),
    ('economy',         'settings.view')
  ) AS v(tab_key, perm_key)
 WHERE e.tab_key = v.tab_key AND e.perm_key IS NULL;

-- ============ 4) 防呆 ============
DO $$
DECLARE
    n int;
    bad text;
BEGIN
    -- 「接近站长」的定义：99 凭 class 拿到的键，98 一把都不能少
    SELECT count(*) INTO n
      FROM role_permissions rp99
     WHERE rp99.role_type = 'class' AND rp99.role_key = '99'
       AND rp99.granted
       AND NOT EXISTS (
         SELECT 1 FROM role_permissions rp98
          WHERE rp98.role_type = 'class' AND rp98.role_key = '98'
            AND rp98.permission_key = rp99.permission_key
            AND rp98.granted);
    IF n > 0 THEN
        RAISE EXCEPTION '98 档仍缺 % 把 99 档 class 权限', n;
    END IF;

    -- 白名单里不得残留 99 档入口
    SELECT count(*) INTO n FROM staff_panel_entries
     WHERE min_class = 99
       AND tab_key IN (
         'deldisabled', 'forums', 'settings', 'mail', 'emailbans',
         'syslog', 'dbstats', 'cleanup', 'bans', 'maxlogin',
         'agentrules', 'locations', 'plugins', 'perm', 'trackers',
         'panelentries', 'onboarding', 'economy', 'ops');
    IF n > 0 THEN
        RAISE EXCEPTION '仍有 % 条导航钉在 99 档', n;
    END IF;

    -- 本次补键的 14 条不得还空着
    SELECT string_agg(tab_key, ', ') INTO bad
      FROM staff_panel_entries
     WHERE perm_key IS NULL
       AND tab_key IN (
         'deldisabled', 'mail', 'emailbans', 'syslog', 'dbstats',
         'cleanup', 'maxlogin', 'locations', 'plugins', 'agentrules',
         'perm', 'trackers', 'onboarding', 'economy');
    IF bad IS NOT NULL THEN
        RAISE EXCEPTION '导航项缺 perm_key: %', bad;
    END IF;

    -- 全表诊断（只告警不阻断）：98 可见、但键只授到 99 的入口
    SELECT string_agg(e.tab_key || '→' || e.perm_key, ', ') INTO bad
      FROM staff_panel_entries e
     WHERE e.min_class <= 98 AND e.perm_key IS NOT NULL
       AND NOT EXISTS (
         SELECT 1 FROM role_permissions rp
          WHERE rp.permission_key = e.perm_key AND rp.granted
            AND rp.role_type = 'class'
            AND CASE WHEN rp.role_key ~ '^[0-9]+$'
                     THEN rp.role_key::int <= 98 ELSE FALSE END);
    IF bad IS NOT NULL THEN
        RAISE NOTICE '98 档带键入口需人工核对: %', bad;
    END IF;
END $$;

COMMIT;
