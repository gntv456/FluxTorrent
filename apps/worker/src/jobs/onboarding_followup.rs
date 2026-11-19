//! 新人生命周期跟进（0503，体检六轮生命周期触点图最高缺口）：
//! 注册第 1-7 天此前完全静默——欢迎 PM 是「用户自己来」，无人主动推。
//! 本 job 每日一档两触达，事实判定全部复用 onboarding_facts 口径：
//!   · day3_nudge：注册满 3 天仍未完成首次下载 → 提醒 PM（新手卡
//!     第一步的主动推送版；流失集中首周，这是最便宜的挽留动作）
//!   · first_seed_grats：首做种达成（seeded_seconds>0 首次落库）→
//!     祝贺 PM + 引导契约
//! 幂等：锚 notice_marker（amount=0）与站内信同语句 CTE 原子落——
//! 重跑天然去重（class_demote 锚的竞态教训：先发信后回锚有窗口）。

use sqlx::PgPool;

pub(crate) async fn onboarding_followup(db: &PgPool) -> anyhow::Result<u64> {
    let mut acted = 0u64;
    // A) day-3 未首下提醒
    let nudged = sqlx::query(
        r#"
        WITH anchor AS (
            INSERT INTO spark_ledger (id, user_id, amount, kind, idempotency_key)
            SELECT nextval('spark_ledger_id_seq'), u.id, 0, 'notice_marker',
                   'onboard-nudge:' || u.id
            FROM users u
            WHERE u.status < 2
              AND u.created_at < now() - interval '3 days'
              AND u.created_at > now() - interval '4 days'
              AND NOT EXISTS (SELECT 1 FROM snatches s
                              WHERE s.user_id = u.id
                                AND s.downloaded > 0
                                AND s.completed_at IS NOT NULL)
              AND NOT EXISTS (SELECT 1 FROM spark_ledger sl
                              WHERE sl.idempotency_key =
                                    'onboard-nudge:' || u.id)
            RETURNING user_id
        )
        INSERT INTO messages (sender_id, receiver_id, subject, body)
        SELECT NULL, a.user_id, '上手小提醒：还没下载第一颗种子？',
               '注册三天啦！资源库里有大量 Free 种（下载不扣分享率），'
               || '挑一颗热门的用客户端导入 .torrent 就算上手成功——'
               || '新手清单与客户端配置指南见「账户健康」页；'
               || '有疑问随时联系管理组。'
        FROM anchor a
        "#,
    )
    .execute(db)
    .await?;
    acted += nudged.rows_affected() as u64;
    // B) 首做种祝贺（近 24h 内首次出现做种时长，注册 ≤14 天）
    let congratulated = sqlx::query(
        r#"
        WITH seeders AS (
            SELECT u.id
            FROM users u
            WHERE u.status < 2
              AND u.created_at > now() - interval '14 days'
              AND EXISTS (SELECT 1 FROM snatches s
                          WHERE s.user_id = u.id AND s.seeded_seconds > 0)
              AND NOT EXISTS (SELECT 1 FROM spark_ledger sl
                              WHERE sl.idempotency_key =
                                    'onboard-seedgrats:' || u.id)
        ),
        anchor AS (
            INSERT INTO spark_ledger (id, user_id, amount, kind, idempotency_key)
            SELECT nextval('spark_ledger_id_seq'), s.id, 0, 'notice_marker',
                   'onboard-seedgrats:' || s.id
            FROM seeders s
            RETURNING user_id
        )
        INSERT INTO messages (sender_id, receiver_id, subject, body)
        SELECT NULL, a.user_id, '里程碑：第一次做种达成！',
               '检测到你的第一颗种子开始做种了——这一步意味着你从下载者'
               || '变成了站点的供给者。持续做种既攒分享率也攒等级；'
               || '注册满 30 天后还可以在「做种契约」页签月度目标拿大额奖励。'
        FROM anchor a
        "#,
    )
    .execute(db)
    .await?;
    acted += congratulated.rows_affected() as u64;
    Ok(acted)
}

/// 0503 运营触达二批（体检台账 #7）：流失梯度 + 注册周年。
/// 此前唯一触点是第 136 天的威胁式休眠警告——30/60 天的软召回窗口
/// 全部错过。锚同款 notice_marker；两档互相独立、各只发一次。
pub(crate) async fn lifecycle_touch(db: &PgPool) -> anyhow::Result<u64> {
    let mut acted = 0u64;
    // A) 30 天未登录软召回（曾活跃过：有下载史，排除已触达）
    let g30 = sqlx::query(
        r#"
        WITH anchor AS (
            INSERT INTO spark_ledger (id, user_id, amount, kind, idempotency_key)
            SELECT nextval('spark_ledger_id_seq'), u.id, 0, 'notice_marker',
                   'lifecycle-g30:' || u.id
            FROM users u
            WHERE u.status < 2
              AND u.last_seen_at < now() - interval '30 days'
              AND u.last_seen_at > now() - interval '34 days'
              AND EXISTS (SELECT 1 FROM snatches s
                          WHERE s.user_id = u.id AND s.downloaded > 0)
              AND NOT EXISTS (SELECT 1 FROM spark_ledger sl
                              WHERE sl.idempotency_key =
                                    'lifecycle-g30:' || u.id)
            RETURNING user_id
        )
        INSERT INTO messages (sender_id, receiver_id, subject, body)
        SELECT NULL, a.user_id, '好久不见——站里有你的种在等你',
               '你已经 30 天没有登录了。你的做种还在为站点供血，'
               || '本周新上了不少 Free 资源——回来逛一圈，签到奖励照旧。'
               || '账号长期未登录会进入休眠流程，提前逛逛就不会有那份尴尬邮件啦。'
        FROM anchor a
        "#,
    )
    .execute(db)
    .await?;
    acted += g30.rows_affected() as u64;
    // B) 60 天未登录第二档（附保种区引导）
    let g60 = sqlx::query(
        r#"
        WITH anchor AS (
            INSERT INTO spark_ledger (id, user_id, amount, kind, idempotency_key)
            SELECT nextval('spark_ledger_id_seq'), u.id, 0, 'notice_marker',
                   'lifecycle-g60:' || u.id
            FROM users u
            WHERE u.status < 2
              AND u.last_seen_at < now() - interval '60 days'
              AND u.last_seen_at > now() - interval '64 days'
              AND NOT EXISTS (SELECT 1 FROM spark_ledger sl
                              WHERE sl.idempotency_key =
                                    'lifecycle-g60:' || u.id)
            RETURNING user_id
        )
        INSERT INTO messages (sender_id, receiver_id, subject, body)
        SELECT NULL, a.user_id, '两个月没见了——别让账号沉睡',
               '60 天未登录，账号正在走向休眠流程（150 天未登录可能被停用）。'
               || '回来看看：保种区正需要人认领濒危种子，签到连签奖励也还在等。'
               || '只要登录一次就不会有任何风险。'
        FROM anchor a
        "#,
    )
    .execute(db)
    .await?;
    acted += g60.rows_affected() as u64;
    // C) 注册周年 PM（满 365 天，窗口 1 天，各周年年份独立锚）
    let anniv = sqlx::query(
        r#"
        WITH anchor AS (
            INSERT INTO spark_ledger (id, user_id, amount, kind, idempotency_key)
            SELECT nextval('spark_ledger_id_seq'), u.id, 0, 'notice_marker',
                   'lifecycle-anniv:' || u.id || ':' ||
                   (floor(extract(epoch FROM (now() - u.created_at)) / 86400) / 365)::int
            FROM users u
            WHERE u.status < 2
              AND u.created_at <= now() - interval '365 days'
              AND date_part('doy', u.created_at) BETWEEN
                  date_part('doy', now()) - 1 AND date_part('doy', now())
              AND NOT EXISTS (
                  SELECT 1 FROM spark_ledger sl
                  WHERE sl.idempotency_key LIKE 'lifecycle-anniv:' || u.id || ':%')
            RETURNING user_id
        )
        INSERT INTO messages (sender_id, receiver_id, subject, body)
        SELECT NULL, a.user_id, '注册周年快乐！',
               '今天是你在站点的注册周年——感谢一路陪伴。'
               || '你贡献的每一颗种、每一条帖都在让站点更好。'
               || '周年之际，欢迎去「做种契约」签一份月度目标，把贡献变成奖励。'
        FROM anchor a
        "#,
    )
    .execute(db)
    .await?;
    acted += anniv.rows_affected() as u64;
    Ok(acted)
}
