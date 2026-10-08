-- 0308（tracker 五轮后续，2026-10-08 站长拍板三件）
-- 依据：_doc/PT-tracker五轮深挖实测审计-2026-10-08.md §十「未做」三条里的两件
-- （第三件 passkey 立即撤销不需要迁移）。
--
-- ① connectable_gate：回连可达性从「一票否决在种」改成可配档位。
--    现状：`process_event` 的 seeding 判据里 `ev.conn != Some(0)` 是硬条件，
--    而主流三家（本轮拉源码核实）都只把 connectable 当**信息位**——
--    UNIT3D `config/announce.php` 的 `connectable_check` 默认 false，唯一的
--    消费者是 BON 积分条件；NexusPHP 建行时硬编码 'yes'、按可达过滤的那句
--    peerlist SQL 是注释掉的；Gazelle 的 `xbt_files_users.connectable` 默认 1
--    且 Ocelot 从不写它。我们的硬否决会把 NAT 后没有映射入站端口的**真做种者**
--    静默判成不在种（在种数、保种考核、濒危种救援一起塌），且用户毫无感知。
--    档位只有两档，不做三档假选择：off（默认）= 不可达不否决在种；
--    hard = 实测不可达即不在种（本站旧行为，想要硬口径的站点显式开）。
--    曾经写过第三档 soft（「只留痕不否决」）并删掉：留痕本来就是
--    `snatches.connectable` 这一列，与 off 的行为没有任何差别——
--    一档没有后果的枚举就是本轮报告一直在报的那种假开关。
--
-- ② ratio_gate：即时分享率闸门。现状是 `user_classes.min_ratio` 与
--    `site_settings.ratiolimit` **全仓零消费者**（本轮 grep 命中 0）= 假开关；
--    活着的是 `ratio_watch` 那条**异步**链（跌破阈值 → 警告 + N 天观察期 →
--    到期仍跌破才 `download_enabled=FALSE, download_locked_by='ratio_watch'`），
--    tracker 侧靠 download_enabled 间接生效。也就是说「低于门槛当场别下载」
--    这件事从来没有人做，而它是 PT 站最核心的准入策略之一
--    （NexusPHP 在 announce 里按比率放行，UNIT3D 走 ratiocheck + 按等级的 min_ratio）。
--    本档补的是**即时**那一层，与 ratio_watch 的柔性观察期并存且互不重复惩罚：
--    观察期内的账号视为已处置，闸门放行。
--
-- ②的安全轨（重要）：本机 `ratiolimit` 现值是 **6**、所有 `user_classes.min_ratio`
--    都是 **0**——直接把 block 打开等于把全站下载锁死。所以
--    · 阈值 = max(等级 min_ratio, 站点 ratiolimit)，再被 `ratio_gate_max` 截断；
--    · `downloaded = 0`（比率无从计算）与注册未过 `ratio_gate_grace_days`
--      /等级 `min_age_days` 的新人，一律不拦；员工（class ≥ 90）不拦；
--    · 缺省档位是 **warn**（只计数与日志，不拦），站长核对过数字再拧 block。
--    截断发生时会打一条 warn，指著说「你填的 6 已被截到 1.0」，
--    而不是安静地按一个荒谬值把人挡在门外。
--
-- 号段说明：本轮开工时 0306 被三路同时占用（本文件 + 站长的
-- 0306_reset_frequency_cap.sql、0306_review_approved_at.sql，后两者已由站长改为
-- 0306/0307）。sqlx 对同版本号不同校验和直接拒绝启动，加迁移前先 ls 这个目录。
-- 全部幂等（IF NOT EXISTS / ON CONFLICT DO NOTHING），重跑安全。

-- ============ ① 回连可达性档位 ============
INSERT INTO site_settings (name, value, descr, grp)
VALUES ('connectable_gate', 'off',
        'tracker 回连 + BT 握手探测结果如何影响「在种」判定：'
        'off=完全不否决在种，实测结果只留在 snatches.connectable 供面板筛（默认，主流口径）；'
        'hard=实测不可达即不计算种（本站旧行为，会误伤 NAT 后的真做种者）。',
        'anticheat')
ON CONFLICT (name) DO NOTHING;

-- 枚举外的存值归回缺省：代码只认 hard，其余一律按 off 走（见 seeding_gate.rs）。
UPDATE site_settings
   SET value = 'off', updated_at = now()
 WHERE name = 'connectable_gate'
   AND value NOT IN ('off', 'hard');

INSERT INTO settings_meta
  (name, type, label_zh, label_en, hint, options, group_key, card_order, visible)
VALUES
  ('connectable_gate', 'enum', '回连可达性档位', 'Connectable gate',
   '探测不可达（conn=0）的 peer 是否还算「在种」。off 只在 snatches.connectable '
   '留痕、面板可筛；hard 才否决在种状态——机房/公网可达的站点可切 hard 强化幽灵做种防线。',
   '[{"l":"不参与判定（默认）","v":"off"},{"l":"不可达即不在种（旧行为）","v":"hard"}]'::jsonb,
   'anticheat', 14, true)
ON CONFLICT (name) DO UPDATE
  SET type = EXCLUDED.type,
      label_zh = EXCLUDED.label_zh,
      label_en = EXCLUDED.label_en,
      hint = EXCLUDED.hint,
      options = EXCLUDED.options,
      group_key = EXCLUDED.group_key,
      card_order = EXCLUDED.card_order,
      visible = EXCLUDED.visible;

-- ============ ② 即时分享率闸门 ============
INSERT INTO site_settings (name, value, descr, grp)
VALUES ('ratio_gate', 'warn',
        '分享率低于门槛时是否当场拒绝「下载」（announce 的 left>0；做种永不拦）：'
        'off=不拦；warn=只留痕与计数（默认，先核对门槛数值）；block=当场拒。'
        '门槛 = max(等级 min_ratio, 站点 ratiolimit)，且被 ratio_gate_max 截断。',
        'anticheat')
ON CONFLICT (name) DO NOTHING;

INSERT INTO settings_meta
  (name, type, label_zh, label_en, hint, options, group_key, card_order, visible)
VALUES
  ('ratio_gate', 'enum', '低分享率闸门', 'Ratio gate',
   '即时闸门，与「分享率观察期」并存：观察期内的账号不再重复处罚。'
   '先设 warn 看计数与日志，确认门槛数值合理后再拧 block。',
   '[{"l":"关闭","v":"off"},{"l":"只留痕不拦（默认）","v":"warn"},{"l":"当场拒绝下载","v":"block"}]'::jsonb,
   'anticheat', 15, true)
ON CONFLICT (name) DO NOTHING;

INSERT INTO site_settings (name, value, descr, grp)
VALUES ('ratio_gate_grace_days', '7',
        '注册未超过该天数的账号不受低分享率闸门约束（新人本来就没有上传量）。'
        '等级自带的 user_classes.min_age_days 取较大者。',
        'anticheat')
ON CONFLICT (name) DO NOTHING;

INSERT INTO settings_meta
  (name, type, label_zh, label_en, hint, unit, group_key, card_order, visible,
   min, max, step)
VALUES
  ('ratio_gate_grace_days', 'number', '闸门新人宽限', 'Ratio gate grace days',
   '按注册天数豁免，0 = 不豁免（新人一注册就被挡，通常不是想要的）。',
   '天', 'anticheat', 16, true, 0, 365, 1)
ON CONFLICT (name) DO NOTHING;

INSERT INTO site_settings (name, value, descr, grp)
VALUES ('ratio_gate_max', '1.0',
        '闸门实际执行门槛的上界：门槛算出来大于它时按它执行并打日志。'
        '防的是历史值/误填（如 ratiolimit=6）一刀切把全站锁死——要真的用高门槛'
        '请显式上调本值，而不是把 ratiolimit 填大。',
        'anticheat')
ON CONFLICT (name) DO NOTHING;

INSERT INTO settings_meta
  (name, type, label_zh, label_en, hint, unit, group_key, card_order, visible,
   min, max, step)
VALUES
  ('ratio_gate_max', 'number', '闸门门槛上界', 'Ratio gate ceiling',
   '实际门槛 = min(max(等级 min_ratio, ratiolimit), 本值)。',
   '比率', 'anticheat', 17, true, 0.01, 20, 0.01)
ON CONFLICT (name) DO NOTHING;

-- ratiolimit 从今天起真的生效：把它的说明改成实际语义，并把它挪进
-- 与闸门同一张卡（此前 group_key 里存的是中文标签「比率限制」，
-- 面板按 group_key 归组 ⇒ 它一直落在任何分组之外，也是一个「看得见名字、
-- 不知道有没有用」的键）。
UPDATE site_settings
   SET descr = '站点级最低分享率（announce 即时闸门的下限来源之一）：'
               '实际门槛 = max(本值, 等级 min_ratio)，再被 ratio_gate_max 截断；'
               '仅在 ratio_gate=warn/block 时参与判定。',
       grp = 'anticheat'
 WHERE name = 'ratiolimit';
UPDATE settings_meta
   SET group_key = 'anticheat', card_order = 18,
       label_zh = '最低分享率（站点级）',
       label_en = 'Minimum ratio (site floor)',
       hint = '与等级 min_ratio 取较大者作为闸门门槛；数值请谨慎，'
              '超过 ratio_gate_max 的部分会被截断。'
 WHERE name = 'ratiolimit';

-- ============ tracker 读口的列（判据仍只有一份：视图） ============
-- 闸门要在 announce 热路径上知道「这个人还有多少量、注册多久、在不在观察期」，
-- 而 passkey → 人 的判据必须继续只有 user_by_passkey 一处（0302 的不变式）。
-- 补四列而不是新写一条查 users 的 SQL：四个读口（tracker/兼容层/RSS）共用同一视图。
CREATE OR REPLACE VIEW user_by_passkey AS
    SELECT id, status, class_id, download_enabled, suspended, passkey,
           uploaded, downloaded, created_at, ratio_watch_until
      FROM users
    UNION ALL
    SELECT id, status, class_id, download_enabled, suspended,
           passkey_prev AS passkey,
           uploaded, downloaded, created_at, ratio_watch_until
      FROM users
     WHERE passkey_prev IS NOT NULL
       AND passkey_prev_until IS NOT NULL
       AND passkey_prev_until > now();

COMMENT ON VIEW user_by_passkey IS
'passkey → 用户的唯一判据（当前钥 + 宽限窗内的旧钥），附带闸门所需的量与观察期标记。'
'tracker / 兼容层 / RSS 共用；status 门槛仍由消费方自己判（tracker 用 status < 2）。';
