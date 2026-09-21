-- 0147 字幕成就族（0146 方案 P2-4：复用 0079 成就体系，挂 3 档里程碑，
-- 不照抄 U3D 的 13 档）。指标 subtitle_count = 公开归属的过审字幕数
--（匿名上传不计入公开归属，与用户主页口径一致）。
INSERT INTO achievement_defs (family, code, name, descr, metric, threshold, reward_sparks, position) VALUES
  ('subtitle', 'sub_1',   '字幕新秀', '公开字幕上传 ≥ 1 条',  'subtitle_count', 1,  100, 1),
  ('subtitle', 'sub_10',  '字幕译者', '公开字幕上传 ≥ 10 条', 'subtitle_count', 10, 1000, 2),
  ('subtitle', 'sub_100', '字幕大师', '公开字幕上传 ≥ 100 条','subtitle_count', 100, 8000, 3)
ON CONFLICT (code) DO NOTHING;
