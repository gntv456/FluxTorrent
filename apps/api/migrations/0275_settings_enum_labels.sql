-- 0275：主要设定三张 enum 卡片「人话化」——选项按钮显示语义名而非裸英文值
--
-- 背景：registration_mode / guest_policy 的 options 一直是裸字符串数组
-- （["invite_only","open",...]），紧凑模式按钮直接渲染英文值；hint 也是
-- 「注册模式：invite_only / open」这种给开发者的口径。站长在后台看到的
-- 是「invite_only」而非「邀请制注册」。
-- 修法：换 [{v,l}] 结构化选项（前端 enumOptions 形态 1、后端 validate_field
-- 的 enum 分支都原生支持，save 路径存 v 不变）；hint 重写为面向站长的说明。
-- 同批把 0236 追加的「仅支持邀请制/开放注册」过时表述一并清掉
-- （email_verify 已于 0236 回补为真实第三档，该句已失真）。

-- ===== 1) 注册模式 =====
UPDATE settings_meta
SET options = $$
 [
   {"v": "invite_only", "l": "邀请制"},
   {"v": "open", "l": "开放注册"},
   {"v": "email_verify", "l": "开放注册 + 邮件激活"}
 ]$$::jsonb,
    hint = '新用户如何注册：邀请制 = 凭邀请码注册；开放注册 = 任何人可直接注册；开放注册 + 邮件激活 = 可直接注册，但须点验证邮件才能登录',
    updated_at = now()
WHERE name = 'registration_mode';

-- ===== 2) 访客可见性 =====
UPDATE settings_meta
SET options = $$
 [
   {"v": "all_private", "l": "全站私密"},
   {"v": "recent_only", "l": "仅最新列表"},
   {"v": "showcase", "l": "橱窗展示"}
 ]$$::jsonb,
    hint = '未登录访客能看到什么：全站私密 = 一律跳登录；仅最新列表 = 可看最新种子列表；橱窗展示 = 公开推荐位',
    updated_at = now()
WHERE name = 'guest_policy';

-- ===== 3) 其余裸字符串 enum 同批人话化 =====
-- 防作弊（0112）
UPDATE settings_meta
SET options = $$
 [
   {"v": "warn", "l": "仅警告"},
   {"v": "limit_download", "l": "暂停下载"}
 ]$$::jsonb,
    updated_at = now()
WHERE name = 'ratio_watch_action';

UPDATE settings_meta
SET options = $$
 [
   {"v": "log", "l": "仅记录"},
   {"v": "warn", "l": "记录并警告"}
 ]$$::jsonb,
    updated_at = now()
WHERE name = 'agent_hit_action';

-- 支付网关（0114）
UPDATE settings_meta
SET options = $$
 [
   {"v": "none", "l": "未启用"},
   {"v": "epay", "l": "易支付"}
 ]$$::jsonb,
    updated_at = now()
WHERE name = 'payment_provider';

-- 字幕区口径（0146）
UPDATE settings_meta
SET options = $$
 [
   {"v": "subtitle", "l": "字幕"},
   {"v": "lyric", "l": "歌词"}
 ]$$::jsonb,
    updated_at = now()
WHERE name = 'subtitle_kind';
