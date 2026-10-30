-- 0434: 摘除两张注册相关死卡——verification / enable_image_verification。
--
-- 背景：注册门控是三代口径叠加的历史包袱。第一代 NP 对齐键（0034/0039）
-- verification（四选一矩阵：关闭/邮箱验证/图形验证码/邮箱+图形）与
-- enable_image_verification（yesno）只做了「登记可写」，从未有过消费者；
-- 真正生效的是第二代自研键 registration_mode（0107，三态枚举，读方在
-- auth_http/register.rs）与第三代验证码驱动键 captcha_provider（0227，
-- none/turnstile/recaptcha/hcaptcha/pow）。旧矩阵的两个维度早已正交拆开：
-- 邮件激活 = registration_mode 的 email_verify 档；验证码形态 =
-- captcha_provider。站长在死卡上怎么切，注册页行为一字不变——与 0193
-- 假开关收口同判据（全仓零读取方）。
--
-- 只摘 meta 行与值行；settings_group_order / i18n 分区名不动（分组还有
-- 8 个真键在用）。fake_switch_baseline.json 也不动：guard 只认 INSERT
-- 登记面，DELETE 摘不掉登记，0193 摘的 5 键至今留在基线里当墓碑，同例。

DELETE FROM settings_meta WHERE name IN ('verification', 'enable_image_verification');
DELETE FROM site_settings WHERE name IN ('verification', 'enable_image_verification');
