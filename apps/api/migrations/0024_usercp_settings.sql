-- 0024: 控制面板用户偏好（复刻 NexusPHP usercp personal/tracker/forum/security 四组设定）
-- Postgres：用 TEXT + CHECK 代替 MySQL ENUM，迁移保持幂等。
ALTER TABLE users ADD COLUMN IF NOT EXISTS parked BOOLEAN NOT NULL DEFAULT FALSE;                        -- 封存账号
ALTER TABLE users ADD COLUMN IF NOT EXISTS accept_pm TEXT NOT NULL DEFAULT 'yes' CHECK (accept_pm IN ('yes','friends','no'));   -- 接受短讯
ALTER TABLE users ADD COLUMN IF NOT EXISTS delete_pm BOOLEAN NOT NULL DEFAULT TRUE;                     -- 回复后删除短讯
ALTER TABLE users ADD COLUMN IF NOT EXISTS save_pm BOOLEAN NOT NULL DEFAULT FALSE;                      -- 保存短讯至发件箱
ALTER TABLE users ADD COLUMN IF NOT EXISTS comment_pm BOOLEAN NOT NULL DEFAULT TRUE;                    -- 种子新评论通知
ALTER TABLE users ADD COLUMN IF NOT EXISTS notify_topic_reply BOOLEAN NOT NULL DEFAULT TRUE;            -- 论坛回复通知
ALTER TABLE users ADD COLUMN IF NOT EXISTS notify_hr BOOLEAN NOT NULL DEFAULT TRUE;                     -- H&R 达标通知
ALTER TABLE users ADD COLUMN IF NOT EXISTS gender SMALLINT NOT NULL DEFAULT 0;                          -- 0未知 1男 2女
ALTER TABLE users ADD COLUMN IF NOT EXISTS country INT NOT NULL DEFAULT 0;                             -- 国家/地区（NexusPHP countries.id 口径）
ALTER TABLE users ADD COLUMN IF NOT EXISTS download_speed INT NOT NULL DEFAULT 0;                       -- 下行带宽 1-18
ALTER TABLE users ADD COLUMN IF NOT EXISTS upload_speed INT NOT NULL DEFAULT 0;                         -- 上行带宽 1-18
ALTER TABLE users ADD COLUMN IF NOT EXISTS isp INT NOT NULL DEFAULT 0;                                  -- ISP 1-6,20
ALTER TABLE users ADD COLUMN IF NOT EXISTS info TEXT;                                                   -- 个人说明（BBCode）
ALTER TABLE users ADD COLUMN IF NOT EXISTS signature TEXT;                                              -- 签名档（BBCode）
-- 网站设定（tracker）
ALTER TABLE users ADD COLUMN IF NOT EXISTS browsecat TEXT;                                              -- 默认分类 CSV（如 "401,403"）
ALTER TABLE users ADD COLUMN IF NOT EXISTS stylesheet TEXT NOT NULL DEFAULT 'BaoziPT';
ALTER TABLE users ADD COLUMN IF NOT EXISTS fontsize TEXT NOT NULL DEFAULT 'medium' CHECK (fontsize IN ('small','medium','large'));
ALTER TABLE users ADD COLUMN IF NOT EXISTS site_language TEXT NOT NULL DEFAULT 'chs';
ALTER TABLE users ADD COLUMN IF NOT EXISTS pm_per_page INT NOT NULL DEFAULT 10;
ALTER TABLE users ADD COLUMN IF NOT EXISTS show_description BOOLEAN NOT NULL DEFAULT TRUE;              -- 详情页显示简介
ALTER TABLE users ADD COLUMN IF NOT EXISTS show_imdb BOOLEAN NOT NULL DEFAULT TRUE;                     -- 详情页显示 IMDb
ALTER TABLE users ADD COLUMN IF NOT EXISTS show_comment BOOLEAN NOT NULL DEFAULT TRUE;                  -- 显示评论
ALTER TABLE users ADD COLUMN IF NOT EXISTS show_ad BOOLEAN NOT NULL DEFAULT TRUE;                       -- 显示广告
ALTER TABLE users ADD COLUMN IF NOT EXISTS time_type TEXT NOT NULL DEFAULT 'timealive' CHECK (time_type IN ('timeadded','timealive'));
ALTER TABLE users ADD COLUMN IF NOT EXISTS torrents_per_page INT NOT NULL DEFAULT 0;                    -- 0=默认 最大100
ALTER TABLE users ADD COLUMN IF NOT EXISTS incl_dead INT NOT NULL DEFAULT 1;                            -- 0包括断种 1活种 2断种
ALTER TABLE users ADD COLUMN IF NOT EXISTS sp_state INT NOT NULL DEFAULT 0;                             -- 促销筛选 0-7
ALTER TABLE users ADD COLUMN IF NOT EXISTS incl_bookmarked INT NOT NULL DEFAULT 0;                      -- 收藏筛选 0/1/2
ALTER TABLE users ADD COLUMN IF NOT EXISTS tooltip TEXT NOT NULL DEFAULT 'off' CHECK (tooltip IN ('minorimdb','medianimdb','off'));
ALTER TABLE users ADD COLUMN IF NOT EXISTS append_sticky BOOLEAN NOT NULL DEFAULT TRUE;                 -- 置顶图标
ALTER TABLE users ADD COLUMN IF NOT EXISTS append_new BOOLEAN NOT NULL DEFAULT TRUE;                    -- '新' 标
ALTER TABLE users ADD COLUMN IF NOT EXISTS append_promotion TEXT NOT NULL DEFAULT 'icon' CHECK (append_promotion IN ('highlight','word','icon','off'));
ALTER TABLE users ADD COLUMN IF NOT EXISTS append_picked BOOLEAN NOT NULL DEFAULT TRUE;                 -- '经典' 标
ALTER TABLE users ADD COLUMN IF NOT EXISTS small_descr BOOLEAN NOT NULL DEFAULT TRUE;                   -- 显示副标题
ALTER TABLE users ADD COLUMN IF NOT EXISTS dl_icon BOOLEAN NOT NULL DEFAULT TRUE;                       -- 下载图标
ALTER TABLE users ADD COLUMN IF NOT EXISTS bm_icon BOOLEAN NOT NULL DEFAULT TRUE;                       -- 收藏图标
ALTER TABLE users ADD COLUMN IF NOT EXISTS show_com_num BOOLEAN NOT NULL DEFAULT TRUE;                  -- 显示评论数
ALTER TABLE users ADD COLUMN IF NOT EXISTS show_last_com TEXT NOT NULL DEFAULT 'no' CHECK (show_last_com IN ('yes','no'));
-- 论坛设定
ALTER TABLE users ADD COLUMN IF NOT EXISTS topics_per_page INT NOT NULL DEFAULT 0;
ALTER TABLE users ADD COLUMN IF NOT EXISTS posts_per_page INT NOT NULL DEFAULT 0;
ALTER TABLE users ADD COLUMN IF NOT EXISTS view_avatars BOOLEAN NOT NULL DEFAULT TRUE;
ALTER TABLE users ADD COLUMN IF NOT EXISTS view_signatures BOOLEAN NOT NULL DEFAULT TRUE;
ALTER TABLE users ADD COLUMN IF NOT EXISTS tt_last_post BOOLEAN NOT NULL DEFAULT FALSE;                 -- 悬浮提示最新帖子
ALTER TABLE users ADD COLUMN IF NOT EXISTS click_topic TEXT NOT NULL DEFAULT 'firstpage' CHECK (click_topic IN ('firstpage','lastpage'));
-- 安全设定
ALTER TABLE users ADD COLUMN IF NOT EXISTS privacy TEXT NOT NULL DEFAULT 'normal' CHECK (privacy IN ('normal','low','strong'));
-- 登录趋势（账户概览 30 天活跃图）
CREATE TABLE IF NOT EXISTS login_events (
  id BIGSERIAL PRIMARY KEY,
  user_id BIGINT NOT NULL REFERENCES users(id) ON DELETE CASCADE,
  created_at TIMESTAMPTZ NOT NULL DEFAULT now()
);
CREATE INDEX IF NOT EXISTS login_events_user_time ON login_events (user_id, created_at);
-- 回填演示数据（幂等：仅当无记录时）
INSERT INTO login_events (user_id, created_at)
SELECT u.id, now() - (random() * interval '25 days')
FROM users u
WHERE NOT EXISTS (SELECT 1 FROM login_events) AND random() < 0.9;
