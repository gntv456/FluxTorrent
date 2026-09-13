-- 0065：批量发放统一工具（好学 increment-bulk.php 口径）
-- 「上传量增减」「魔力增减」两个页签合并为一个「批量发放」，并支持按等级/职务/指定用户批量。

-- 导航合并：删除旧的两个入口，登记新入口
DELETE FROM staff_panel_entries WHERE tab_key IN ('bonus', 'upload');

INSERT INTO staff_panel_entries (panel, name, url, info, sort, section, min_class, tab_key) VALUES
('admin', '批量发放', '/admin?tool=incrementbulk',
 '按等级/职务/指定用户批量增减火花、上传量、邀请、补签卡（好学 increment-bulk 口径）',
 3, 'users', 93, 'incrementbulk')
ON CONFLICT (tab_key) WHERE tab_key <> '' DO NOTHING;
