-- 0451: 官种名单的变更时机补全（0449 只挂了 AFTER UPDATE）
--
-- 0449 的取舍写在注释里：「写这张表的入口有六处，逐处挂钩子必漏」，所以把
-- 重算挂在 site_settings 的触发器上。那个判断是对的，覆盖是缺一块的：
-- 触发器只有 AFTER UPDATE，于是两类写法旁路得干干净净——
--
--   1. **删行**。`DELETE FROM site_settings WHERE name=$1` 在册两处
--      （settings_http/pack_import.rs 的快照差集清理、pack_store.rs 的
--      theme 还原）。站长或内容包把 official_team_ids 整行删掉 = 「名单清空
--      = 只认标签」，列却不动：靠名单当官种的种子会继续挂着官种，
--      促销 scope='official'、首页统计、NexusPHP 兼容口全跟着错。
--      而这条路径删完还会 INSERT 回来（ON CONFLICT DO UPDATE 才触发），
--      「删→插」中间那段窗口里列值是旧的。
--   2. **插新行**。第一次给这个键写值若走 INSERT（新站装包、直接建表数据），
--      ON CONFLICT DO NOTHING 命中已有行时压根不算 UPDATE，也不触发。
--
-- 修法沿用 0449 自己的口径：把「什么时候该重算」留在数据层，不逐个写入口
-- 挂钩子。PG16 的 WHEN 子句里拿不到 TG_OP，所以按事件拆两条触发器共用一个
-- 函数，函数里用 TG_OP 分辨：
--   · INSERT：无条件重算（新值可能非空，宁可多算一次全表）
--   · UPDATE：只有 value 真变了才算（保留 0449 的幂等口径，避免每次
--     `UPDATE site_settings SET grp=...` 都拖着全站跑一遍）
--   · DELETE：无条件重算（此时名单解析为空集 = 只认标签）
-- INSERT/UPDATE 那条引用 NEW，DELETE 那条引用 OLD，各自都在事件合法的
-- 一侧，不会撞 plpgsql 的「record is not assigned yet」。

DROP TRIGGER IF EXISTS site_settings_official_teams ON site_settings;

CREATE OR REPLACE FUNCTION trg_official_teams_refresh() RETURNS trigger
LANGUAGE plpgsql AS $$
BEGIN
  IF TG_OP = 'UPDATE' AND NEW.value IS DISTINCT FROM OLD.value THEN
    PERFORM refresh_official_tag(NULL);
  ELSIF TG_OP IN ('INSERT', 'DELETE') THEN
    PERFORM refresh_official_tag(NULL);
  END IF;
  RETURN NULL;  -- AFTER 触发器，返回值被忽略
END $$;

COMMENT ON FUNCTION trg_official_teams_refresh() IS
  '官种名单（site_settings.official_team_ids）增/改/删后全量重算 official_tag';

CREATE TRIGGER site_settings_official_teams_insupd
  AFTER INSERT OR UPDATE ON site_settings
  FOR EACH ROW WHEN (NEW.name = 'official_team_ids')
  EXECUTE FUNCTION trg_official_teams_refresh();

CREATE TRIGGER site_settings_official_teams_del
  AFTER DELETE ON site_settings
  FOR EACH ROW WHEN (OLD.name = 'official_team_ids')
  EXECUTE FUNCTION trg_official_teams_refresh();

-- 出厂重算一次：把历史上可能存在的「删行/插行留下的旧列值」对齐当前名单。
SELECT refresh_official_tag(NULL);
