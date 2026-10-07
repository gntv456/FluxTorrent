-- 0302 tracker 深挖审计（2026-10-07）：passkey 宽限窗 + 附加 tracker 凭据开关
-- 依据：_doc/PT-tracker深挖实测审计报告-2026-10-07.md P1-4 / P1-7 / P2（段封禁）
--
-- P1-4：passkey 被烤进用户已下载的每一个 .torrent。改密或「重置密钥」即刻换钥，
--       等于让用户手上所有种子集体停种（libtorrent 系客户端还会把 tracker 标成
--       错误、长时间不再重试），而用户完全看不出原因。旧钥进宽限窗（默认 7 天，
--       PASSKEY_GRACE_HOURS 可调），窗口内照常可用。
-- P1-7：后台「附加 Tracker」旧版对每条地址无条件拼 /announce/<passkey>，
--       站长填进友站或公网 tracker 就会把全体用户凭据写进下发的 .torrent 并持续外发。
--       with_passkey 把「携带本站凭据」变成显式意图（默认 false）。
-- 判据只有一份：所有「按 passkey 找人」的读口改走视图 user_by_passkey，
--       避免 tracker 已放行而 RSS/兼容层仍 401 的半生效状态。

ALTER TABLE users
    ADD COLUMN IF NOT EXISTS passkey_prev text,
    ADD COLUMN IF NOT EXISTS passkey_prev_until timestamptz;

-- 旧钥同样是对外凭据，查找走索引；一个旧值只可能属于一个账号
CREATE UNIQUE INDEX IF NOT EXISTS users_passkey_prev_key
    ON users (passkey_prev)
    WHERE passkey_prev IS NOT NULL;

ALTER TABLE tracker_urls
    ADD COLUMN IF NOT EXISTS with_passkey
        boolean NOT NULL DEFAULT false;

CREATE OR REPLACE VIEW user_by_passkey AS
    SELECT id, status, class_id, download_enabled, suspended, passkey
      FROM users
    UNION ALL
    SELECT id, status, class_id, download_enabled, suspended,
           passkey_prev AS passkey
      FROM users
     WHERE passkey_prev IS NOT NULL
       AND passkey_prev_until IS NOT NULL
       AND passkey_prev_until > now();

COMMENT ON VIEW user_by_passkey IS
    'passkey → 用户的唯一判据（当前钥 + 宽限窗内的旧钥）。tracker / 兼容层 / RSS 共用';
