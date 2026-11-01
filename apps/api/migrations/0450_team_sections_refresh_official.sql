-- 0450: torrent_sections 的 team 维度变更也重算 official_tag
--
-- 0449 把官种口径收进 refresh_official_tag（official 标签 OR 制作组命中
-- official_team_ids 名单），但只挂了两类重算时机：站长改名单（site_settings
-- 触发器）与标签写入（sync_official_tag）。制作组维度自身的三条写入路径
-- ——发种（store_sections_tags）、编辑（torrent_http/manage.rs）、批量改维
-- （change_sections）——都不重算，深测抓到两个断链病形：
--
--   1. 名单内制作组的新种若不带标签，永远不会标官种（发种路径只有在写
--      标签时才顺带 sync_official_tag）；
--   2. 编辑/批量改维度把制作组改出/改进名单，物化列不动，直到下一次
--      名单变更或标签操作才「突然」对齐。
--
-- 修法沿用 0449 自己的哲学（「写这张表的入口有六处，逐处挂钩子必漏」）：
-- 把重算挂到 torrent_sections 上。PG16 的 WHEN 里拿不到 TG_OP，一条多事件
-- 触发器无法在 DELETE 事件安全引用 NEW——按事件拆成两条触发器共用同一个
-- 函数：INSERT/UPDATE 看 NEW.kind，DELETE 看 OLD.kind，team 维度单行触发、
-- 只重算受影响种子。AFTER 行级触发器在同语句内逐行生效，全维重建
-- （DELETE+INSERT）的编辑口径下最后一个 team 行落定后列值即正确；非 team
-- 维度被 WHEN 挡住零开销。tags 表保持 Rust 侧 sync_official_tag 不变
-- （0449 已收口，且 tags 无 kind='team' 行，挂触发器无收益）。

CREATE OR REPLACE FUNCTION trg_team_refresh_official() RETURNS trigger
LANGUAGE plpgsql AS $$
BEGIN
  PERFORM refresh_official_tag(
           COALESCE(NEW.torrent_id, OLD.torrent_id));
  RETURN NULL;  -- AFTER 触发器，返回值被忽略
END $$;

DROP TRIGGER IF EXISTS torrent_sections_team_official ON torrent_sections;
CREATE TRIGGER torrent_sections_team_official_ins
  AFTER INSERT OR UPDATE ON torrent_sections
  FOR EACH ROW WHEN (NEW.kind = 'team')
  EXECUTE FUNCTION trg_team_refresh_official();

CREATE TRIGGER torrent_sections_team_official_del
  AFTER DELETE ON torrent_sections
  FOR EACH ROW WHEN (OLD.kind = 'team')
  EXECUTE FUNCTION trg_team_refresh_official();
