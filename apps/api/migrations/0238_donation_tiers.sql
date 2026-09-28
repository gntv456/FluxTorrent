-- E7 捐赠自动回馈档位：
-- ① payment_orders.tier_granted——档位发放 CAS 闸（照抄 shop_orders.effect_applied
--    范式 0087：回调与手动补单两入口共用，置位成功才发放，重放天然跳过）；
-- ② settings 键 donation_tiers（grp=payment，JSON textarea）——档位配置
--    [{min_usd, label, spark, upload_gb, invites, medal_id}]，解析失败=无档位；
-- ③ 通知偏好白名单键 donate_tier（notice.rs KEYS + 前端 notice-prefs 同步）。

ALTER TABLE payment_orders
    ADD COLUMN IF NOT EXISTS tier_granted BOOLEAN NOT NULL DEFAULT FALSE;

INSERT INTO site_settings (name, value, descr, grp)
VALUES ('donation_tiers', '[]',
        '捐赠回馈档位（JSON 数组：min_usd/label/spark/upload_gb/invites/medal_id，空=不启用档位回馈）',
        'payment')
ON CONFLICT (name) DO UPDATE SET descr = EXCLUDED.descr;

INSERT INTO settings_meta (name, type, label_zh, label_en, group_key, card_order)
VALUES ('donation_tiers', 'textarea',
        '捐赠回馈档位（累计实付达档自动发放魔力/上传量/邀请）',
        'Donation reward tiers (cumulative paid)',
        'payment', 6)
ON CONFLICT (name) DO UPDATE SET label_zh = EXCLUDED.label_zh;
