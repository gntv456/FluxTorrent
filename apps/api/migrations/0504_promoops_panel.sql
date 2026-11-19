-- 0504 运营工作台导航行（体检七轮运营台账 #1/#2/#5 收口配套）：
-- admin-promo-ops 面板（促销归因/自动规则/竞赛 CRUD）staffpanel 导航。
INSERT INTO staff_panel_entries
    (panel, name, url, info, sort, section, min_class, tab_key, perm_key)
SELECT 'sysop', '运营工作台', '/admin?tool=promoops',
       '促销效果归因 / 过审自动促销规则 / 竞赛管理', 16, 'ops', 98,
       'promoops', 'settings.manage'
 WHERE NOT EXISTS (
     SELECT 1 FROM staff_panel_entries WHERE tab_key = 'promoops'
);
