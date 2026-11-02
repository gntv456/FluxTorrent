-- 0453: 制作组按标题派生（0449 官种名单的前提条件）
--
-- 0449 让站长可以指定「哪几个制作组算官种」，0450 把制作组维度的写路径接上
-- 了重算。但现网实测：torrent_sections 的 team 维**挂载 0 行**（字典 70 项
-- 一项都没被用过）⇒ 名单勾得再准，派生也永远只剩「official 标签」那一条腿，
-- 站长看到的就是「改了没反应」。缺的不是判定规则，是**数据**。
--
-- 两个缺口：
--   1. 存量种从来没挂过组（制作组维度是 0441 才切成字典 select 的，且
--      section_kinds.team 的 required=false，会员可以不填）；
--   2. 增量也不保证：非必填 → 大多数人不填。
--
-- 所以这里把「从标题认制作组」做成**一把尺**（一个 SQL 函数），回填和发种
-- 共用它——写成两份迟早分叉（本仓反复抓到的病形）。规则刻意保守：
--   · R1 结尾必须是 `-<组名>`（发布组的通行命名：Xxx.2026.1080p.WEB-DL.x264-HDSky）
--   · R2 或以 `[组名]` / `【组名】` / `(组名)` 整块出现（字幕组风格）
-- 两个规则都是**字面比较**（right()/position()），不走正则也不走 LIKE：
-- 字典名里带 `-`、空格，LIKE 的 `%`/`_` 与正则元字符都要额外转义，而 PG 没有
-- regexp_quote；用字面函数就没有这条漏（历史上 LIKE 忘转义把通配符当字面用，
-- 本仓立过常驻闸门）。
--   · 多个组名同时命中 → 取**最长**的那个（"HXWEB" 赢过 "HX"）
--   · 种子已经有 team 行就**不动**：会员/站长手选的一律优先于派生
--   · 挂上去的行在 value 里打来源戳（backfill / derive），要撤就按戳撤
--
-- 现网实测命中率：305 颗种认出 4 颗（全是 -FLUX），零误伤——这批语料本来就
-- 以闸门夹具为主（AuditB.Free.* / Audit.Pack.深测），夹具的 `-GROUP`、`WEB-DL`
-- 这些技术串不在字典里，尺子不会硬认。真实发布语的站库里覆盖率会高得多。
--
-- 挂组走的是 torrent_sections 的 INSERT ⇒ 0450 的触发器自动重算 official_tag，
-- 这里不另调 refresh_official_tag（口径只留一份）。

-- 派生 + 挂载（发种路径与回填共用同一支函数）
CREATE OR REPLACE FUNCTION attach_team_from_name(
    p_torrent_id BIGINT,
    p_source TEXT DEFAULT 'derive') RETURNS BOOLEAN
LANGUAGE plpgsql AS $$
DECLARE
  nm  TEXT;
  tid BIGINT;
BEGIN
  IF NOT EXISTS (SELECT 1 FROM torrents WHERE id = p_torrent_id) THEN
    RETURN false;
  END IF;
  -- 已有 team 行 = 人选的（或上次派生的），不覆盖
  IF EXISTS (SELECT 1 FROM torrent_sections
              WHERE torrent_id = p_torrent_id AND kind = 'team') THEN
    RETURN false;
  END IF;
  SELECT name INTO nm FROM torrents WHERE id = p_torrent_id;

  SELECT d.id INTO tid
    FROM section_dict d
   WHERE d.kind = 'team'
     AND ( right(lower(nm), length(lower(d.name)) + 1) = '-' || lower(d.name)
        OR position(lower('[' || d.name || ']')  in lower(nm)) > 0
        OR position(lower('【' || d.name || '】') in lower(nm)) > 0
        OR position(lower('(' || d.name || ')')  in lower(nm)) > 0 )
   ORDER BY length(d.name) DESC, d.id
   LIMIT 1;

  IF tid IS NULL THEN
    RETURN false;
  END IF;

  INSERT INTO torrent_sections (torrent_id, kind, dict_id, ordinal, value)
       VALUES (p_torrent_id, 'team', tid,
               COALESCE((SELECT max(ordinal) + 1 FROM torrent_sections
                          WHERE torrent_id = p_torrent_id
                            AND kind = 'team'), 1),
               jsonb_build_object('source', p_source));
  RETURN true;
END $$;

COMMENT ON FUNCTION attach_team_from_name(BIGINT, TEXT) IS
  '按标题字面匹配制作组并挂载（已有 team 行不覆盖）；命中返回 true';

-- 存量回填：只碰一颗颗没挂过组的种，函数本身幂等（重跑零新增）
SELECT count(*) FILTER (WHERE attached) AS backfilled
  FROM (SELECT attach_team_from_name(id, 'backfill') AS attached
          FROM torrents ORDER BY id) x;

-- 增量：挂在 torrents 的 INSERT 上，而不是在发种代码里叫一嗓子。
-- 理由与 0449 同源——造种子的入口不止一条：
--   POST /torrents（本站上传）、NP 兼容口 upload.php、np_import 批量导入、
--   内容包/站型包写入、求种转正 offer_promote。逐处挂必漏，漏掉的那条路径
--   发出来的种就永远不挂组，官种名单对它永久失效。
-- 时序上这是安全的：发种写维度走 publish_http::upload_sections::write_one，
-- 它对**本次提交了的维度**先 `DELETE ... WHERE kind=$2` 再重建 ⇒ 会员显式选的
-- 制作组会覆盖这条派生行；没提交 team 维时派生行保留，正是要的效果。
-- 派生行只在「当前没有任何 team 行」时插入（函数头部判据），不会双挂。
CREATE OR REPLACE FUNCTION trg_torrent_team_derive() RETURNS trigger
LANGUAGE plpgsql AS $$
BEGIN
  PERFORM attach_team_from_name(NEW.id, 'derive');
  RETURN NULL;  -- AFTER 行级触发器，返回值被忽略
END $$;

DROP TRIGGER IF EXISTS torrents_team_derive ON torrents;
CREATE TRIGGER torrents_team_derive
  AFTER INSERT ON torrents
  FOR EACH ROW EXECUTE FUNCTION trg_torrent_team_derive();
