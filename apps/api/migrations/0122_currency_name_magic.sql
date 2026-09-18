-- 0122_currency_name_magic.sql
-- 默认货币名统一为「魔力」：清理**种子数据 / 设置元数据**里残留的「火花」。
--
-- 背景：站点货币名由 site_settings.currency_name 决定（默认「魔力」），前端走 {magic} 占位符动态替换。
-- 但 0004/0016/0039/0109 等种子迁移把「火花」写进了**人类可读的标签/单位/描述**里，
-- 这些列不经占位符替换，会原样显示在商店、投票、后台站点设定页。
--
-- 边界（有意为之）：
--   * 只改「用户可见文案」列；
--   * **不碰 messages**（439+10 行是已投递的历史站内信，不重写历史记录）；
--   * **不碰** spark_ledger / spark_balance / gift_spark / reward_sparks 等内部标识与列名；
--   * 不改任何**已执行过**的迁移文件（sqlx 校验 checksum），只在此处做数据订正。
--
-- 幂等：全部按 LIKE '%火花%' 过滤 + replace()，重复执行无副作用。

UPDATE shop_items          SET name     = replace(name,     '火花', '魔力') WHERE name     LIKE '%火花%';
UPDATE fun_polls           SET question = replace(question, '火花', '魔力') WHERE question LIKE '%火花%';
UPDATE modules             SET descr    = replace(descr,    '火花', '魔力') WHERE descr    LIKE '%火花%';
UPDATE settings_meta       SET label_zh = replace(label_zh, '火花', '魔力') WHERE label_zh LIKE '%火花%';
UPDATE settings_meta       SET hint     = replace(hint,     '火花', '魔力') WHERE hint     LIKE '%火花%';
UPDATE settings_meta       SET unit     = replace(unit,     '火花', '魔力') WHERE unit     LIKE '%火花%';
UPDATE site_settings       SET descr    = replace(descr,    '火花', '魔力') WHERE descr    LIKE '%火花%';
UPDATE staff_panel_entries SET info     = replace(info,     '火花', '魔力') WHERE info     LIKE '%火花%';

-- 例外订正：0082 给 currency_name 写的 hint 本身是**示例列表**「如：魔力 / 火花 / 猫粮」，
-- 上面的全局 replace 会把它改成重复的「魔力 / 魔力 / 猫粮」。这里显式改回不重复的示例。
UPDATE settings_meta
   SET hint = '全站显示的货币名（如：魔力 / 猫粮 / 金币），2-12 个字符'
 WHERE name = 'currency_name';
