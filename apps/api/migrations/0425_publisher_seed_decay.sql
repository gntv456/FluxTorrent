-- 0425 发布者做种三档时间衰减（同类站对标 A- 批）。
--
-- 同类站 口径：发布人为自己的种子做种，24h 内 10 分/h、30 天内 5 分/h、
-- 365 天内 1 分/h（相对基础 1 分/h 的 10x/5x/1x 加成）。落到我们
-- seeding 结算的形态：对「做种人是发布者」的行，per-torrent bonus 乘一个
-- 发布者乘数——种龄 <24h ×4 / <30d ×2.5 / <365d ×1.5 / 更久 ×1
-- （默认档，site_settings 三键可调 1.0=关）。
--
-- 为什么乘在 per-torrent 而不是 per-user：发布者加成是对「这颗种」的
-- 激励（首发黄金期把种子保住），乘在用户总额会把「发 1 颗 + 挂 100 颗
-- 他人种」的全额放大，语义完全漂掉。
--
-- 实现为独立函数 + worker 结算 SQL 的一个乘数项，不改 0136 的
-- seeding_torrent_bonus（公式本体单一真相源原则；本函数只做「乘数」）。

INSERT INTO site_settings (name, value, descr, grp) VALUES
  ('seeding_publisher_mult_day',  '4.0', '发布者为自发种做种：种龄<24h 的加成倍数（1.0=关）', 'seeding'),
  ('seeding_publisher_mult_month','2.5', '发布者为自发种做种：种龄<30天 的加成倍数（1.0=关）', 'seeding'),
  ('seeding_publisher_mult_year', '1.5', '发布者为自发种做种：种龄<365天 的加成倍数（1.0=关）', 'seeding')
ON CONFLICT (name) DO NOTHING;

CREATE OR REPLACE FUNCTION seeding_publisher_mult(
    p_is_publisher boolean,
    p_torrent_age_days double precision
) RETURNS double precision LANGUAGE sql STABLE AS $$
    SELECT CASE
        WHEN NOT p_is_publisher THEN 1.0
        WHEN p_torrent_age_days < 1.0 THEN
            COALESCE((SELECT value FROM site_settings
                      WHERE name = 'seeding_publisher_mult_day')::double precision, 4.0)
        WHEN p_torrent_age_days < 30.0 THEN
            COALESCE((SELECT value FROM site_settings
                      WHERE name = 'seeding_publisher_mult_month')::double precision, 2.5)
        WHEN p_torrent_age_days < 365.0 THEN
            COALESCE((SELECT value FROM site_settings
                      WHERE name = 'seeding_publisher_mult_year')::double precision, 1.5)
        ELSE 1.0
    END
$$;

COMMENT ON FUNCTION seeding_publisher_mult IS '发布者做种三档衰减乘数（0425 同类站对标）：仅对 owner 自己的种子生效，档位随种龄衰减，1.0=关。';

-- 0425 附加：弹性校验开关（同类站对标 A-：截图/简介/MediaInfo/外部 ID 四选一）
INSERT INTO site_settings (name, value, descr, grp)
VALUES ('upload_either_or', 'no',
        '发种内容门槛四选一（yes=简介≥50字/有截图/有MediaInfo/有IMDb豆瓣TMDB任一达标即过）',
        'upload')
ON CONFLICT (name) DO NOTHING;

-- 0425 附加：热搜词统计（同类站对标 A-）。联想接口顺手 UPSERT 计数，
-- 首页读 TOP N。口径：只记「有结果的查询」（搜索成功才计入，防止
-- 乱敲键盘污染词云）；词长 2-64；保留 90 天窗口。
CREATE TABLE IF NOT EXISTS search_term_stats (
    term TEXT PRIMARY KEY,
    hits BIGINT NOT NULL DEFAULT 0,
    last_at TIMESTAMPTZ NOT NULL DEFAULT now()
);
CREATE INDEX IF NOT EXISTS search_term_stats_hot
    ON search_term_stats (hits DESC, last_at DESC)
    WHERE last_at > now() - interval '90 days';
