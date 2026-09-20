-- 勋章稀有度词表（0143）：把原先散在前端硬编码的稀有度词表（legendary/epic/rare/common）
-- 落到库里，让站长能在后台自建/改名/删除，前台与后台都读这一份。
--
-- tone 只存"配色档"标识（前端固定 6 档 → tailwind token 类），不往库里写 CSS，
-- 避免站长输入任意样式类把主题体系打破。
CREATE TABLE IF NOT EXISTS medal_rarities (
    value      TEXT PRIMARY KEY,                    -- 落到 medals.rarity 里的键（英文 slug）
    label      TEXT NOT NULL,                       -- 前台显示名
    tone       TEXT NOT NULL DEFAULT 'sky',         -- 配色档：gold/coral/mint/sky/indigo/candy
    sort       INTEGER NOT NULL DEFAULT 0,          -- 排序（小组在前）
    updated_at TIMESTAMPTZ NOT NULL DEFAULT now()
);

-- 初装种子 = 改造前前端那 4 个值，颜色与"传说金/史诗靛/稀有蓝/普通绿"保持一致
INSERT INTO medal_rarities (value, label, tone, sort) VALUES
  ('legendary', '传说', 'gold',   10),
  ('epic',      '史诗', 'indigo', 20),
  ('rare',      '稀有', 'sky',    30),
  ('common',    '普通', 'mint',   40)
ON CONFLICT (value) DO NOTHING;
