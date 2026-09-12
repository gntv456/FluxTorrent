-- 修正 implemented 标记：see_banned 已于代码层接线（种子列表 include_unapproved 参数），
-- 0057 建标记时它尚未接线故标为 false，此处更正为 true。
-- 注意：0057 已应用不可修改（sqlx 校验和），故在新迁移中更正。

UPDATE permissions SET implemented = true WHERE key = 'torrent.see_banned';
