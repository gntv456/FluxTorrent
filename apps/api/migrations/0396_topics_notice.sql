-- 0396_topics_notice.sql
-- 主题「公告」标记（对齐样图集 § tplForum 的 公告 徽章 + 只看公告 筛选）。
--
-- 背景：样图话题行有三种标记——公告（coral 徽章 + 珊瑚色行底）、
-- 置顶（indigo 徽章 + sky 行底）、精华（sun 徽章 + 琥珀行底），
-- 筛选行也是四档（全部话题 / 只看公告 / 只看置顶 / 只看精华）。
-- 本站此前只有 sticky/digest 两个布尔列，没有公告位，筛选行只能三档。
--
-- 纯加列、默认 false ⇒ 存量主题行为零变化；不建索引——公告筛选是
-- 版主/读者的低频过滤，topics 量级远未到需要部分索引的地步。

ALTER TABLE topics ADD COLUMN IF NOT EXISTS notice boolean NOT NULL DEFAULT false;
