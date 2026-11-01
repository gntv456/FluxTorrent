-- 0449: 官种可以按「制作组名单」自动判定
--
-- 现状：官种是纯布尔物化列 torrents.official_tag（0001 建列），值只由
-- 「挂没挂 official-kind 标签」派生（0159 收口，唯一写口 manage.rs 的
-- sync_official_tag）。站长现在要的是：**我指定哪几个制作组算官种**，
-- 这些组发的种自动是官种，并且列表要有独立筛选。
--
-- 口径选择：名单只作为 official_tag 的**第二个派生条件**，不新增一条
-- 平行的「按组判官种」查询谓词。理由——official_tag 被促销 scope
-- （promo.rs）、发布自动置顶插件（plugins_builtin）、首页统计（home.rs）、
-- NexusPHP 兼容口（nexusphp.rs）四处直接读，任何「查询层另算一套」都会
-- 立刻造成口径分叉（这类洞本仓已立过常驻闸门）。
-- 因此把派生收进 SQL 函数 refresh_official_tag：Rust 侧只调它，站长改
-- 名单时全量重算一次，列与标签/制作组三者恒一致。
--
-- 名单存 site_settings.official_team_ids（逗号分隔的 section_dict ID，
-- grp=torrent）：存 ID 不存名字，制作组改名不断链（0441 的转种口径就是
-- 「按字典 ID，text+字典会坏」）。非法/越界 token 在解析时丢掉——这个值
-- 可能被内容包导入或改库留下脏数据，宁可少判一个组也不能让派生语句报错。

INSERT INTO site_settings (name, value, grp) VALUES
  ('official_team_ids', '', 'torrent')
ON CONFLICT (name) DO NOTHING;

INSERT INTO settings_meta
  (name, type, label_zh, label_en, hint, unit, min, max, step, options,
   group_key)
VALUES
  ('official_team_ids', 'text', '官种制作组名单',
   'Official release groups',
   '逗号分隔的制作组字典 ID（「维度字典 → 制作组」里的 ID）。名单内的制作组'
   '发布的种子自动算官种，与「手工打官种标签」并列生效；清空 = 只认标签。'
   '保存后全站重算一次 official_tag。',
   '', NULL, NULL, NULL, '{"rule":"csv_ids"}'::jsonb, 'torrent')
ON CONFLICT (name) DO NOTHING;

UPDATE site_settings SET grp = 'torrent'
 WHERE name = 'official_team_ids'
   AND grp IS DISTINCT FROM 'torrent';

-- 名单解析：非数字/空串一律丢弃，返回空数组表示「只认标签」
CREATE OR REPLACE FUNCTION official_team_ids_arr() RETURNS BIGINT[]
LANGUAGE sql STABLE AS $$
  SELECT COALESCE(
           array_agg(DISTINCT (NULLIF(BTRIM(x), ''))::bigint),
           '{}'::bigint[]
         )
    FROM unnest(
           string_to_array(
             COALESCE(
               (SELECT value FROM site_settings
                 WHERE name = 'official_team_ids'), ''),
             ',')) AS x
   WHERE NULLIF(BTRIM(x), '') ~ '^[0-9]+$'
$$;

-- 官种派生（唯一口径）：official-kind 标签 OR 制作组在名单内。
-- p_torrent_id 为 NULL 时全量重算（站长改名单后调用），返回受影响行数。
CREATE OR REPLACE FUNCTION refresh_official_tag(
    p_torrent_id BIGINT DEFAULT NULL) RETURNS INTEGER
LANGUAGE plpgsql AS $$
DECLARE
  n INTEGER := 0;
BEGIN
  UPDATE torrents t
     SET official_tag = EXISTS (
           SELECT 1 FROM tags tg JOIN tag_dict d ON d.id = tg.tag_id
            WHERE tg.torrent_id = t.id AND d.kind = 'official')
        OR EXISTS (
           SELECT 1 FROM torrent_sections ts
            WHERE ts.torrent_id = t.id
              AND ts.kind = 'team'
              AND ts.dict_id = ANY (official_team_ids_arr()))
   WHERE p_torrent_id IS NULL OR t.id = p_torrent_id;
  GET DIAGNOSTICS n = ROW_COUNT;
  RETURN n;
END $$;

COMMENT ON FUNCTION refresh_official_tag(BIGINT) IS
  '官种物化列唯一派生入口：official 标签 OR 制作组命中 official_team_ids';

-- 出厂重算一次（此时名单为空，等价于把列与标签重新对齐，无行为变化）
SELECT refresh_official_tag(NULL);

-- 名单变更即重算：挂在 site_settings 上的触发器，而不是逐个设置保存口。
-- 理由：写这张表的入口有六处（PUT /admin/settings/groups、PUT /admin/settings、
-- 安装向导、运营向导、站型包导入、以及直接改库），逐处挂钩子必漏，且「只在
-- 某个口生效」正是本仓反复抓到的病形（配置存了、回显了，但没有执行路径读它）。
-- AFTER UPDATE 才看得到新值，official_team_ids_arr() 因此读到的是改后的名单。
CREATE OR REPLACE FUNCTION trg_official_teams_refresh() RETURNS trigger
LANGUAGE plpgsql AS $$
BEGIN
  IF NEW.name = 'official_team_ids'
     AND NEW.value IS DISTINCT FROM OLD.value THEN
    PERFORM refresh_official_tag(NULL);
  END IF;
  RETURN NEW;
END $$;

DROP TRIGGER IF EXISTS site_settings_official_teams ON site_settings;
CREATE TRIGGER site_settings_official_teams
  AFTER UPDATE ON site_settings
  FOR EACH ROW EXECUTE FUNCTION trg_official_teams_refresh();
