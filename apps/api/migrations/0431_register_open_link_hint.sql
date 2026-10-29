-- 0431: register_open 提示语补联动说明（P-680 配套）。
--
-- 「开放注册」（register_open，NP yesno 口径）与「注册模式」
-- （registration_mode，三态枚举）此前零联动：站长打开「开放注册」，
-- 注册页仍按 invite_only 强制邀请码。自本批起保存 register_open 会
-- 自动同步 registration_mode（yes→open / no→invite_only），提示语
-- 把这层关系说破，避免两张卡片两张脸。

UPDATE settings_meta
   SET hint = '快捷开关：开启 = 注册模式切为「开放注册」（注册不需要邀请码）；关闭 = 切回「邀请制」。要与「开放注册 + 邮件激活」等细粒度模式，请直接改「注册模式」'
 WHERE name = 'register_open';
