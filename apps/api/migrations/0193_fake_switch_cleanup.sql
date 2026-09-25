-- 0193: 假开关收口（四审 L2/L6/L8）。
--
-- 1) 摘除「登记在设置页但全仓零读取方」的五个键。判据是 grep：api 源码与 web
--    组件/lib 里没有任何读取点。留着的后果是站长改完数字以为生效。
--    同期核过的另外两键 **不摘**，因为它们已在本批接上真实读取：
--      attendance_first / attendance_streak / attendance_daily_cap
--        → economy::CheckInParams::from_settings（economy_http/checkin.rs 调用）
--      farm_market_window_hours
--        → games_http::helpers::farm_market_hours（三处农场窗口计算调用）
--      bank_max_rate_pct     —— 无任何展示/计算读取点（实际利率走规则表达式）
--      magic_pool_target_default —— 建池与展示都不读它
--      carousel_images       —— 首页无轮播渲染能力，键形同虚设
--      stylesheet_default / nfo_view_style_default —— 用户偏好未实现
DELETE FROM settings_meta WHERE name IN (
    'carousel_images', 'stylesheet_default', 'nfo_view_style_default',
    'bank_max_rate_pct', 'magic_pool_target_default'
);
DELETE FROM site_settings WHERE name IN (
    'carousel_images', 'stylesheet_default', 'nfo_view_style_default',
    'bank_max_rate_pct', 'magic_pool_target_default'
);
-- 站型包 economy 预设同步剔掉这两个已摘除的键：apply_pack_extras 虽然有
-- settings_meta 白名单守卫不会写入，但包里留着会让 diff/审计报出无关改动。
UPDATE site_type_packs
SET economy = economy - 'bank_max_rate_pct' - 'magic_pool_target_default'
WHERE economy IS NOT NULL
  AND (economy ? 'bank_max_rate_pct' OR economy ? 'magic_pool_target_default');

-- 2) 主题令牌改回取色器控件（0191 登记成 text，逼站长手输 #rrggbb；
--    setting-field-controls.tsx 早有 color 分支）。
UPDATE settings_meta
SET type = 'color', updated_at = now()
WHERE name LIKE 'theme\_token\_%';

-- 3) site_type 标记只读（二审 B-4 已让两个 PUT 端点拒收该键并引导走站型切换，
--    但设置页仍把它渲染成可改下拉——改了必失败，且 options 是 0039 硬写的 11 个
--    内置 code，custom_* 永不出现）。前端 readonly 分支见 setting-field-controls.tsx:79。
UPDATE settings_meta
SET readonly = true,
    hint = '站型由「站型包」页切换（apply 站型包）来改变，此处直改会被拒绝',
    updated_at = now()
WHERE name = 'site_type';
