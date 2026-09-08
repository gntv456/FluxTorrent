-- M14 勋章种子数据（81 枚基准口径起步集：开站/节气/运营系列）
INSERT INTO medals (name, price, rarity, limited) VALUES
  ('开站勋章',     NULL,   'legendary', true),
  ('开学先锋',     20000,  'epic',      true),
  ('放暑假啦',     15000,  'rare',      true),
  ('春分',         8000,   'rare',      false),
  ('夏至',         8000,   'rare',      false),
  ('秋分',         8000,   'rare',      false),
  ('冬至',         8000,   'rare',      false),
  ('保种达人',     30000,  'epic',      false),
  ('发布标兵',     30000,  'epic',      false),
  ('签到满月',     10000,  'rare',      false),
  ('签到百日',     50000,  'epic',      false),
  ('农场主',       12000,  'rare',      false),
  ('学习之星',     5000,   'common',    false),
  ('种子守护者',   20000,  'epic',      false)
ON CONFLICT DO NOTHING;

-- M15 论坛版块（旧站 9 版块口径）
INSERT INTO forums (name, descr, min_class) VALUES
  ('站务公告',   '站点公告与规则发布',         0),
  ('资源交流',   '资源求助与交流',             0),
  ('新手报到',   '新会员自我介绍',             0),
  ('播种帮助',   '做种/下载技术支持',          0),
  ('课本讨论',   '教材与学习方法交流',         0),
  ('活动专区',   '站点活动与福利',             0),
  ('议事厅',     '站点发展建议',               0),
  ('水族馆',     '闲聊灌水',                   0),
  ('回收站',     '归档帖',                     90)
ON CONFLICT DO NOTHING;
