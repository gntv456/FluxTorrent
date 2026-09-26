-- 0208：闭环审查 P0/P1 批（2026-09-26 审查报告落地）
--
-- P0-5：email_verify 注册模式从「空壳」变「诚实」——枚举里移除该选项，
--   只留 invite_only / open（实现里 email_verify 与 open 行为一致且不发验证信；
--   留着选项 = 站长以为有邮箱门槛实际没有）。存量站点若已选 email_verify
--   归一为 open（行为本来等同 open）。
-- P1-6：捐赠订单后台管理需要的状态口径（paid_at 手工补单时已有列，无新列）。
-- P1-7：staff_panel_entries CRUD 无新列（权限过滤按现有 perm_key 语义扩）。
-- P2-14：metadata_sources 增加 'mediainfo' 开关口径（不发新键，运行时按数组判断）。
-- P2-12：开站 checklist 三键已存在（announce_url / smtp_host / registration_mode），
--   无新键；checklist 接口运行时聚合判断。

-- ===== P0-5 =====
UPDATE settings_meta
SET options = '{"options": ["invite_only", "open"]}'::jsonb,
    hint = COALESCE(hint, '') || CASE WHEN hint LIKE '%email_verify%'
        THEN '' ELSE ' 仅支持邀请制/开放注册' END
WHERE name = 'registration_mode';

UPDATE site_settings
SET value = 'open'
WHERE name = 'registration_mode' AND value = 'email_verify';

-- ===== P2-19：备份命令可配置 =====
INSERT INTO site_settings (name, value, descr, grp) VALUES
('backup_docker_exec', 'docker exec flux-postgres pg_dump -U flux fluxtorrent',
 '备份命令（默认走 docker；裸机部署改为本地 pg_dump 全路径）', 'misc')
ON CONFLICT (name) DO NOTHING;

INSERT INTO settings_meta
    (name, type, label_zh, label_en, hint, group_key, card_order)
VALUES
('backup_docker_exec', 'text', '备份命令', 'Backup command',
 '后台「备份」按钮执行的完整命令（shell）；默认 docker exec 容器内 pg_dump，非 compose 部署请改为本地 pg_dump 路径',
 'misc', 90)
ON CONFLICT (name) DO NOTHING;

-- ===== P2-17：多语言切换器可隐藏 =====
INSERT INTO site_settings (name, value, descr, grp) VALUES
('locale_switcher_enabled', 'yes', '显示语言切换器', 'misc')
ON CONFLICT (name) DO NOTHING;

INSERT INTO settings_meta
    (name, type, label_zh, label_en, hint, group_key, card_order)
VALUES
('locale_switcher_enabled', 'yesno', '显示语言切换器', 'Show locale switcher',
 '关闭后隐藏顶栏语言切换下拉（单语站点）；站点仍支持 ?locale= cookie 切换',
 'misc', 91)
ON CONFLICT (name) DO NOTHING;
