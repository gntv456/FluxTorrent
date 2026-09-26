-- 0214：闭环五路方案修复批（P0/P1/P2，2026-09-26 复核附记落地）
--
-- P0-2.2 注册死锁：向导完成后注册模式为 invite_only 时站点无法产生第二个
--   用户——向导侧自动产首码（setup_http.rs 改动，无新键）。
-- P0-2.1/2.3：announce_url 采集与 SMTP 测试端点为纯代码改动，无新键。
-- P1-3.6 附件单文件上限可配：attach_max_mib（缺省 8 = 既有硬编码口径，
--   行为零变化；0=不限）。
-- P2-4.4 JWT 有效期可配：jwt_ttl_hours（缺省 24 = 既有口径；cookie max-age
--   同步读取同键）。
-- P2-4.5 邀请码强度：new_invite_code 改 crypto RNG（代码改动，无新键）。
-- P1-3.5 插件运行时开关：plugin__{name} 键约定（分发处热读，30s 缓存；
--   不逐插件种键——站长在设定页自建 plugin__auto_pin_official=no 即可停用，
--   未建键 = 缺省启用，行为零变化）。
-- P2-4.6 站点图标：site_favicon（URL；空 = 内置 icon），sitetype 下发 +
--   layout.tsx icons 接线。

-- ===== P1-3.6 附件单文件上限 =====
INSERT INTO site_settings (name, value, descr, grp) VALUES
('attach_max_mib', '8', '单文件附件上限（MiB，0=不限）', 'attachment')
ON CONFLICT (name) DO NOTHING;

INSERT INTO settings_meta
    (name, type, label_zh, label_en, hint, group_key, card_order)
VALUES
('attach_max_mib', 'number', '单文件上限（MiB）', 'Max file size (MiB)',
 '上传附件的单文件大小上限；0 = 不限。视频走独立的 video_quota_mib 配额',
 'attachment', 92)
ON CONFLICT (name) DO NOTHING;

-- ===== P2-4.4 JWT 有效期 =====
INSERT INTO site_settings (name, value, descr, grp) VALUES
('jwt_ttl_hours', '24', '登录令牌有效期（小时）', 'security')
ON CONFLICT (name) DO NOTHING;

INSERT INTO settings_meta
    (name, type, label_zh, label_en, hint, group_key, card_order)
VALUES
('jwt_ttl_hours', 'number', '登录令牌有效期（小时）', 'Login token TTL (hours)',
 'JWT 与登录 cookie 的有效期；缩短可减小令牌泄露窗口，但用户需更频繁重新登录（1-720）',
 '登录安全', 90)
ON CONFLICT (name) DO NOTHING;

-- ===== P2-4.6 站点图标 =====
-- grp 用 appearance（与 site_logo 同区），卡片用「SEO 与统计」与 META 三键同卡
INSERT INTO site_settings (name, value, descr, grp) VALUES
('site_favicon', '', '站点图标 URL（空 = 内置）', 'appearance')
ON CONFLICT (name) DO NOTHING;

INSERT INTO settings_meta
    (name, type, label_zh, label_en, hint, group_key, card_order)
VALUES
('site_favicon', 'text', '站点图标 URL', 'Site favicon URL',
 '浏览器标签页图标；支持 /api/v1/attachments/{sha} 站内附件地址或外链；留空用内置图标',
 'SEO 与统计', 95)
ON CONFLICT (name) DO NOTHING;
