-- 0353 登录 PoW 闸门难度可配（sha256-pow-v1）。
-- 依据：连续登录失败第 3 次起要求客户端完成 SHA-256 工作量证明（详见
-- apps/api/src/gaps_http/pow.rs）。难度此前只能硬编码 18 bit——开源后所有
-- 站同参，脚本编写者可按最低难度预制算力。与 0311 反作弊阈值同一条纪律：
-- 算法公开、参数进 site_settings。
--
-- 幂等（ON CONFLICT DO NOTHING），重跑安全。

INSERT INTO site_settings (name, value, descr, grp)
VALUES ('pow_difficulty_bits', '18',
        '登录工作量证明难度（bit）：连续失败后每次登录尝试须先算出前 N bit '
        '为 0 的 SHA-256 摘要。出厂默认 18（浏览器约秒级），范围 12-24，'
        '调大提高爆破成本也增加正常用户等待。',
        'security')
ON CONFLICT (name) DO NOTHING;

INSERT INTO settings_meta
  (name, type, label_zh, label_en, hint, unit, group_key, card_order,
   visible, min, max, step)
VALUES
  ('pow_difficulty_bits', 'number', '登录 PoW 难度', 'Login PoW difficulty',
   '连续登录失败后要求的工作量证明难度（bit）。18 ≈ 浏览器秒级；每 +1 bit '
   '成本翻倍。过低起不到拦爆破作用，过高会拖慢正常用户。',
   'bit', 'security', 9, true, 12, 24, 1)
ON CONFLICT (name) DO NOTHING;
