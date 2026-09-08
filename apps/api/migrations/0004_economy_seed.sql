-- M11/M12 经济系统种子数据（旧站商品全集口径，方案 §4.2）
INSERT INTO shop_items (name, kind, price, config) VALUES
  ('1GB 上传量',      'upload_credit',  3000,  '{"gb": 1}'),
  ('5GB 上传量',      'upload_credit', 12000,  '{"gb": 5}'),
  ('10GB 上传量',     'upload_credit', 20000,  '{"gb": 10}'),
  ('100GB 上传量',    'upload_credit',150000,  '{"gb": 100}'),
  ('邀请名额',        'invite',         50000,  '{}'),
  ('临时邀请名额',    'temp_invite',    20000,  '{}'),
  ('自定义头衔',      'custom_title',   80000,  '{}'),
  ('贵宾待遇',        'vip',           100000,  '{}'),
  ('APP 专属 VIP(30天)','app_vip',      60000,  '{}'),
  ('赠送火花',        'gift_spark',     10000,  '{"spark": 1000}'),
  ('15 天去广告',     'ad_free',        10000,  '{}'),
  ('补签卡',          'makeup_card',     5000,  '{}'),
  ('彩虹 ID',         'rainbow_id',     30000,  '{}'),
  ('改名卡',          'rename_card',    10000,  '{}'),
  ('头像框',          'avatar_frame',   20000,  '{}'),
  ('彩虹用户名样式',  'rainbow_name',   15000,  '{}'),
  ('动态头像',        'animated_avatar',25000,  '{}'),
  ('慈善捐赠',        'charity',         1000,  '{}')
ON CONFLICT DO NOTHING;

-- 旧站银行利率口径（7/30/90/180/365 天）
INSERT INTO jixiao_types (name, metrics, base_pay) VALUES
  ('保种员',   '{"seed_size_tb": 5, "seed_days": 25}', 5000),
  ('发布员',   '{"uploads": 20}',                      8000),
  ('转种员',   '{"uploads": 50, "seed_size_tb": 2}',   6000),
  ('维护开发员','{"ops": 100}',                         20000),
  ('主管',     '{"ops": 500}',                         50000)
ON CONFLICT DO NOTHING;
