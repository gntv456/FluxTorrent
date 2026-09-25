-- 0196：给后台面板条目挂模块键（四审 L5 P0「关掉模块后后台面板不跟着消失」）
--
-- 29 个可选模块在前台四处一致（导航/页面/API/worker），但后台面板表只有
-- min_class 一个维度，于是中立站关掉 exams/jixiao/farm 之后，后台仍挂
-- 「考核配置 / 绩效考核 / 月度授金」等条目 —— 点进去接口才被网关拒。
-- 本迁移把明显从属某模块的面板条目绑到模块键上；未列出的条目（分类/维度/
-- 标签/自定义字段/自定义页面/菜单/审核/举报/审计等）属建站核心，恒显示。
ALTER TABLE staff_panel_entries
    ADD COLUMN IF NOT EXISTS module_key text;

UPDATE staff_panel_entries SET module_key = CASE tab_key
    WHEN 'exams'     THEN 'exams'        -- 考核配置
    WHEN 'jixiao'    THEN 'jixiao'       -- 绩效考核
    WHEN 'medals'    THEN 'medals'       -- 勋章管理
    WHEN 'attendance' THEN 'attendance'  -- 签到记录
    WHEN 'tasks'     THEN 'tasks'        -- 任务配置
    WHEN 'subawards' THEN 'subtitles'    -- 金字字幕评选
    WHEN 'props'     THEN 'shop'         -- 道具管理
    WHEN 'polls'     THEN 'forums'       -- 投票（论坛玩法）
    WHEN 'seedstats' THEN 'preserve'     -- 保种统计
    ELSE NULL
    END
WHERE tab_key IN ('exams', 'jixiao', 'medals', 'attendance', 'tasks',
                  'subawards', 'props', 'polls', 'seedstats');

-- 防呆：模块键必须真存在于注册表，否则拼错会让条目永久消失且无线索
DO $$
DECLARE
    bad text;
BEGIN
    SELECT string_agg(DISTINCT module_key, ', ') INTO bad
    FROM staff_panel_entries e
    WHERE e.module_key IS NOT NULL
      AND NOT EXISTS (SELECT 1 FROM modules m WHERE m.key = e.module_key);
    IF bad IS NOT NULL THEN
        RAISE EXCEPTION 'staff_panel_entries.module_key 指向不存在的模块: %', bad;
    END IF;
END $$;
