-- M22 任务种子（旧站「精进研习社」口径示例）
INSERT INTO tasks (name, metric, starts_at, ends_at, target_class, reward, penalty, claim_limit) VALUES
  ('精进研习社 · 做种积分 +1000', '{"seed_points_delta": 1000}', now() - interval '1 day', now() + interval '30 days', 0, 2000, 500, 100),
  ('本月发布 5 枚种子', '{"uploads": 5}', now() - interval '1 day', now() + interval '30 days', 3, 5000, 0, 50),
  ('字幕翻译志愿行', '{"subtitles": 3}', now() - interval '1 day', now() + interval '30 days', 0, 1500, 0, 200)
ON CONFLICT DO NOTHING;
