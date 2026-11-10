-- 0485: 制作组派生尺放宽——「- 组名」（连字符后带空格）也算 R1 结尾。
--
-- 背景（2026-11-11 好学站官种对账实测）：官组名单（official_team_ids）落地
-- 后清点，仍有 6 颗「… pdf - HX」「… pdf - HXWEB」结尾的种既没挂组也没官标
-- ——0453 的 R1 只认 right(nm, len+1) = '-' || name，「- HX」（连字符后带一个
-- 空格，教材类发布的通行写法）不命中。存量靠迁移导入按旧站 team 字段写的行
-- 兜住了大头，但 0453 这把尺管着增量发种（触发器挂在 torrents INSERT 上，
-- 上传/NP 兼容口/批量导入/求种转正共用），同形态的新种会继续漏。
--
-- 放宽（刻意保守）：
--   · R1 允许连字符后**至多一个空格**：right(nm, len+1) = '-' || name
--     OR right(nm, len+2) = '- ' || name（后者同时覆盖「- HX」与「 - HX」）；
--   · 不放宽无连字符形态（「xxx HX」）：子串误伤面不可控，宁缺勿滥；
--   · R2 整块形态（[组名]/【组名】/(组名)）不动；
--   · 多命中取最长、已有 team 行不覆盖、value 打来源戳三条语义全保留。
--
-- 幂等：函数 OR REPLACE（签名与 0453 逐字一致，replace 才合法）；存量回填
-- 重放一遍——函数对「已有 team 行」零写入，重跑无新增时是纯读。

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
        OR right(lower(nm), length(lower(d.name)) + 2) = '- ' || lower(d.name)
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
  '按标题字面匹配制作组并挂载（R1 放宽：-组名 / - 组名；已有 team 行不覆盖）；命中返回 true';

-- 存量回填重放：只碰一颗颗没挂过组的种（0485 前的好学库实测 = 6 颗，
-- 「- HX / - HXWEB」形态；重放幂等，无匹配零新增）
SELECT count(*) FILTER (WHERE attached) AS backfilled
  FROM (SELECT attach_team_from_name(id, 'backfill') AS attached
          FROM torrents ORDER BY id) x;
