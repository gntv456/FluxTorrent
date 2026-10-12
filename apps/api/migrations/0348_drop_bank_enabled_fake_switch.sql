-- 0348: 摘除 bank_enabled 假开关（批三十二 P-87）。
--
-- 0048 起它就在后台「经济」分组里可见可改（yesno「启用银行系统」），但全仓
-- 零读取方——站长关掉它，银行照常放款收息，与 P-59(enableattach) 同型。
-- 银行域的真开关是模块注册表的 module_bank（网关 /api/v1/bank/*、导航、
-- worker 三处均已消费）。按 0193 先例摘除，保留单一事实源；
-- 存量值删除是行为中性的（它此前就没有任何行为）。
DELETE FROM settings_meta WHERE name = 'bank_enabled';
DELETE FROM site_settings WHERE name = 'bank_enabled';
