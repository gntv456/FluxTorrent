-- 作品条目缓存（本地缓存外部元数据）：作品聚合的「作品层」字段正式来源。
-- 口径见 publish_http/work_meta.rs：TMDB details（zh-CN）拉一次存一行，
-- 作品页零外呼；导演/主演/类型/时长/地区/评分都从这里来。
-- key 与作品聚合键同口径（torrent_http/works_sql.rs 的 WK：i/m/d/g/n 前缀），
-- 只有带外部 ID 的键会被回查（g/n 只有站方手工填的组，无外部锚点）。

CREATE TABLE IF NOT EXISTS work_meta (
  key         text PRIMARY KEY,
  source      text NOT NULL DEFAULT 'tmdb',
  external_id text,
  name        text,
  year        int,
  rating      numeric(3,1),
  runtime     int,
  region      text,
  director    text[],
  cast_list   text[],
  genres      text[],
  poster      text,
  synopsis    text,
  fetched_at  timestamptz NOT NULL DEFAULT now()
);

COMMENT ON TABLE work_meta IS
  '作品条目缓存：TMDB details 拉取的结构化元数据，作品详情页单源';
