-- 0099 论坛精华帖（NP digest 口径）：topics.digest 布尔列，版主经 topic manage 设置
ALTER TABLE topics ADD COLUMN IF NOT EXISTS digest boolean NOT NULL DEFAULT false;
