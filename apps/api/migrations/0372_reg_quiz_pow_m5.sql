-- 0372_reg_quiz_pow_m5.sql：M5 答题注册题库 + PoW 验证码 + 规则准确率。
-- 幂等，可重跑。

-- ① 注册答题题库（单选；抽题/判分在 gaps_http/reg_quiz.rs）
CREATE TABLE IF NOT EXISTS reg_quiz_questions (
  id bigserial PRIMARY KEY,
  category text NOT NULL DEFAULT 'general',
  question text NOT NULL,
  -- 选项数组：["选项A","选项B",...]，判分为下标比对
  options jsonb NOT NULL,
  answer_index integer NOT NULL,
  explanation text,
  enabled boolean NOT NULL DEFAULT true,
  created_by bigint,
  created_at timestamptz NOT NULL DEFAULT now(),
  CONSTRAINT reg_quiz_answer_range CHECK (answer_index >= 0)
);
-- 抽题只取启用行（random() 排序 + 该部分索引）
CREATE INDEX IF NOT EXISTS idx_reg_quiz_pick
  ON reg_quiz_questions (category) WHERE enabled;

-- ② 答题留痕（IP 只存 sha3 摘要，与 reg_ip_signups 同口径）
CREATE TABLE IF NOT EXISTS reg_quiz_attempts (
  id bigserial PRIMARY KEY,
  ip_hash text,
  question_id bigint,
  passed boolean,
  answer_index integer,
  created_at timestamptz NOT NULL DEFAULT now()
);
CREATE INDEX IF NOT EXISTS idx_reg_quiz_attempts_created
  ON reg_quiz_attempts (created_at);

-- ③ 内置题库：PT 站规则常识（站点无关；站长可禁用/扩充）
INSERT INTO reg_quiz_questions
  (category, question, options, answer_index, explanation)
SELECT * FROM (VALUES
  ('ratio', '分享率（Ratio）的计算方式是？',
   '["上传量 ÷ 下载量","下载量 ÷ 上传量","做种数 ÷ 下载数","积分 ÷ 流量"]'::jsonb,
   0, '分享率 = 上传量 ÷ 下载量，是 PT 站衡量贡献的核心指标'),
  ('ratio', '下载量很大而上传量很小时，分享率会怎样？',
   '["升高","降低","不变","先升后降"]'::jsonb,
   1, '分母变大，分享率下降；需要多做种回补'),
  ('hr', 'H&R（Hit and Run）指的是？',
   '["下载后未达到规定做种时长/分享率就撤种","上传速度过快","重复下载同一资源","种子被删除"]'::jsonb,
   0, 'H&R 会被扣分甚至封禁，务必按站规做种满时长'),
  ('hr', '避免 H&R 最常见的做法是？',
   '["下载完立刻停止做种","做种达到站点要求的最短时长或分享率","删除种子文件","关闭客户端"]'::jsonb,
   1, '多数站点要求做种满一定时长或单种分享率达标'),
  ('seed', '「保种」指的是？',
   '["长期做种为他人提供下载","把种子备份到网盘","收藏资源","定期更换客户端"]'::jsonb,
   0, '保种是 PT 社区的生命线，也是魔力/积分的主要来源'),
  ('client', '关于 BT 客户端，正确做法是？',
   '["使用站点允许的规范客户端","同时开多个客户端规避限制","修改客户端标识伪装成其他客户端","使用禁用的吸血客户端"]'::jsonb,
   0, '白名单/黑名单由站点维护，伪装客户端属违规'),
  ('cheat', '同一网络下注册多个账号互刷上传量属于？',
   '["正常操作","作弊（Self-deal），会被封号","提升分享率的技巧","站规允许"]'::jsonb,
   1, '同 IP 双账号同 swarm 一传一下会被反作弊系统识别'),
  ('cheat', '以下哪种行为会被判定为作弊？',
   '["正常的做种上传","虚报上传量/伪造下载进度","按站规发布资源","参与论坛讨论"]'::jsonb,
   1, '虚报流量触发流量台账与反作弊校验'),
  ('free', '「免费种」（Freeleech）指的是？',
   '["下载不计入下载量","上传不计入上传量","不需要做种","可以任意转载"]'::jsonb,
   0, '免费种下载不扣流量，是做种攒上传的好机会'),
  ('rule', '发布资源时应？',
   '["按站点分类规范填写信息、上传合规种子文件","随意填写标题以吸引点击","上传他人未授权的付费内容","重复发布同一资源"]'::jsonb,
   0, '违规发布会被删种并可能扣分'),
  ('invite', '关于邀请码，正确的是？',
   '["可自由公开分享给任何人","被邀请人违规时邀请人可能连带受罚","邀请码可重复使用","邀请码永久有效"]'::jsonb,
   1, '邀请制下邀请人需对被邀请人的行为负责'),
  ('report', '发现问题资源（违规/死种/举报）应当？',
   '["使用举报功能并说明原因","在论坛公开辱骂发布者","私下交易","无视即可"]'::jsonb,
   0, '举报走站内流程，由管理组核实处理'),
  ('account', '账号安全方面，正确做法是？',
   '["与其他站点使用相同弱密码","妥善保管密码，发现异常及时通知管理组","把账号借给他人使用","多设备同时登录不设任何保护"]'::jsonb,
   1, '共享账号与弱密码是盗号与关联封禁的高发原因'),
  ('score', '魔力/积分的主要获取途径通常是？',
   '["做种、签到、完成任务与参与社区活动","无限下载资源","重复注册账号","向管理组索取"]'::jsonb,
   0, '做种收益是魔力最稳定的来源')
) AS v(category, question, options, answer_index, explanation)
WHERE NOT EXISTS (SELECT 1 FROM reg_quiz_questions);

-- ④ settings：答题注册 + PoW + 规则准确率窗口
INSERT INTO site_settings (name, value, descr) VALUES
('reg_quiz_enabled', 'no', '注册答题闸：yes 时注册前必须通过一题规则测验'),
('reg_quiz_category', '', '注册答题抽题分类（空 = 全部分类随机）'),
('reg_quiz_ttl_minutes', '10', '注册答题票据有效期（分钟，一次性）'),
('reg_quiz_pass_required', 'yes', '答题必须答对才允许注册（no = 仅记录不拦截）'),
('pow_difficulty_bits', '18', '工作量证明难度（bit，12-24）；captcha_provider=pow 时注册也要解 PoW'),
('cheat_accuracy_days', '90', '规则准确率统计的默认回看天数（反作弊规则质量面板）')
ON CONFLICT (name) DO NOTHING;

-- ⑤ settings_meta：后台编辑面
INSERT INTO settings_meta
  (name, type, label_zh, label_en, hint, unit, min, max, step,
   options, group_key, card_order, min_class)
VALUES
  ('reg_quiz_enabled', 'yesno', '开启注册答题', 'Enable signup quiz',
   '开启后注册前需答对一题站规测验（题库见「注册答题题库」）',
   NULL, NULL, NULL, NULL, NULL, '注册与邀请', 18, 99),
  ('reg_quiz_category', 'text', '答题分类', 'Quiz category',
   '留空 = 全部分类随机抽题；填分类名则只从该分类抽',
   NULL, NULL, NULL, NULL, NULL, '注册与邀请', 19, 99),
  ('reg_quiz_ttl_minutes', 'number', '答题票据有效期',
   'Quiz ticket TTL', '票据一次性，过期作废', '分钟', 1, 1440, 1, NULL,
   '注册与邀请', 20, 99),
  ('reg_quiz_pass_required', 'yesno', '答题必须答对',
   'Quiz pass required', '关闭则只记录答题结果、不拦截注册',
   NULL, NULL, NULL, NULL, NULL, '注册与邀请', 21, 99),
  ('cheat_accuracy_days', 'number', '规则准确率回看天数',
   'Rule accuracy window',
   '反作弊规则质量面板默认统计窗口', '天', 1, 3650, 1, NULL,
   'anticheat', 32, 99)
ON CONFLICT (name) DO UPDATE SET
  type = EXCLUDED.type, label_zh = EXCLUDED.label_zh,
  label_en = EXCLUDED.label_en, hint = EXCLUDED.hint, unit = EXCLUDED.unit,
  min = EXCLUDED.min, max = EXCLUDED.max, step = EXCLUDED.step,
  options = EXCLUDED.options, group_key = EXCLUDED.group_key,
  card_order = EXCLUDED.card_order, min_class = EXCLUDED.min_class,
  updated_at = now();

-- pow_difficulty_bits 是 0353 既有键（group_key/标签/排序都已定），这里
-- **只补 hint**，不重设 group_key/card_order——否则会把它从「安全设定」
-- 第 9 位挪到末尾，属回归。
UPDATE settings_meta SET
  hint = '工作量证明前导零位数；越大越难。captcha_provider=pow 时注册亦适用',
  updated_at = now()
WHERE name = 'pow_difficulty_bits';

UPDATE site_settings SET grp = '注册与邀请'
WHERE name IN ('reg_quiz_enabled', 'reg_quiz_category',
               'reg_quiz_ttl_minutes', 'reg_quiz_pass_required')
  AND (grp IS NULL OR grp <> '注册与邀请');
UPDATE site_settings SET grp = 'anticheat'
WHERE name = 'cheat_accuracy_days'
  AND (grp IS NULL OR grp <> 'anticheat');

-- ⑥ captcha_provider 增列 PoW 选项（自托管，无第三方依赖；login 侧早已
--    支持 pow_verify，注册侧在 0372 一并接入）
UPDATE settings_meta SET
  options = '{"options":[{"label":"自研算术题","value":"none"},'
            '{"label":"工作量证明（PoW）","value":"pow"},'
            '{"label":"Cloudflare Turnstile","value":"turnstile"},'
            '{"label":"reCAPTCHA v2","value":"recaptcha"},'
            '{"label":"hCaptcha","value":"hcaptcha"}]}'::jsonb,
  updated_at = now()
WHERE name = 'captcha_provider'
  AND NOT (options::text LIKE '%"pow"%');
