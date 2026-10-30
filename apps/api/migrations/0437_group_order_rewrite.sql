-- 0437: settings_group_order 全量重写（B2）。
--
-- 背景：该键只登记过 NP 十二分区；0107 后新增的分区（注册与邀请/
-- publishing/anticheat/economy/module 系/games/ops/payment/appearance/
-- auto_review/forum）全部漏登，schema.rs 把它们按发现序追加在尾部
-- （字典序），左导航次序每站漂移。meta.rs 的 DEFAULT_GROUP_ORDER 已同批
-- 重写为同一业务序；本迁移把存量站的值行也拉齐——仅当现值等于旧内置序
-- 才覆盖（站长手工自定义过的顺序保留）。

UPDATE site_settings
   SET value = 'basic,main,注册与邀请,publishing,torrent,attachment,forum,bonus,economy,account,authority,security,smtp,anticheat,auto_review,payment,ops,module,module_fun,module_community,module_economy,games,appearance,advertisement,tweak,misc'
 WHERE name = 'settings_group_order'
   AND value = 'basic,main,smtp,security,authority,tweak,bonus,account,torrent,attachment,advertisement,misc';
