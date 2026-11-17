//! 用户维度统计的 SQL 唯一居所（FRAME §2：H 层禁 sqlx，SQL 只住 repo/）。
//!
//! 背景：插件生态（PT-Plugin-Plus / PT-depiler 口径）要三样东西，而它们的
//! 页面口径端点（/me/spark、/classes）只认登录 JWT，开放 API Token 取不到：
//!   - 发布数（uploads）：本人名下种子数；
//!   - 时魔（每小时火花）：做种收益预估，口径与 worker 结算同源；
//!   - 等级升级规则：class_rules 门槛四维 + 本人现状。
//! 这里把可直查的 SQL 下沉到 R 层，H 层只调函数；口径与页面端点保持一致。

use serde::Serialize;

use super::Repo;

/// 等级门槛一行（user_classes × class_rules 联查，与页面 /classes 同口径）。
///
/// 门槛四维：上传量 / 下载量 / 做种小时 / 账号天数。
/// `has_rule=false` 表示该档在 class_rules 里没有规则行（捐赠/任命发放的档位），
/// 四维是占位 0 —— 消费方画阶梯时不应把它当成人人都能上的台阶。
#[derive(Debug, Clone, Serialize)]
pub struct LevelRule {
    pub id: i32,
    pub name: String,
    pub has_rule: bool,
    pub min_uploaded: i64,
    pub min_download_count: i64,
    pub min_seed_hours: i64,
    pub min_account_age_days: i64,
    pub demotable: bool,
}

/// 本人四维现状（页面 /classes 的 me_stats 同口径：算「距下一级还差多少」）。
#[derive(Debug, Clone, Serialize)]
pub struct MeStats {
    pub uploaded: i64,
    pub download_count: i64,
    pub seed_hours: i64,
    pub account_age_days: i64,
}

/// 成长线等级门槛（class_id < 90 的职务级另走 staff_classes）。
pub async fn level_rules(repo: &Repo) -> Vec<LevelRule> {
    let rows: Vec<(i32, String, i64, i64, i64, i64, bool, bool)> =
        sqlx::query_as(
            "SELECT uc.id, uc.name, \
                COALESCE(cr.min_uploaded, 0)::bigint, \
                COALESCE(cr.min_download_count, 0)::bigint, \
                COALESCE(cr.min_seed_hours, 0)::bigint, \
                COALESCE(cr.min_account_age_days, 0)::bigint, \
                COALESCE(cr.demotable, false), \
                (cr.class_id IS NOT NULL) AS has_rule \
         FROM user_classes uc \
         LEFT JOIN class_rules cr ON cr.class_id = uc.id \
         WHERE uc.id < 90 \
         ORDER BY uc.id",
        )
        .fetch_all(&repo.read_db)
        .await
        .unwrap_or_default();
    rows.into_iter()
        .map(
            |(
                id,
                name,
                min_uploaded,
                min_download_count,
                min_seed_hours,
                min_account_age_days,
                demotable,
                has_rule,
            )| {
                LevelRule {
                    id,
                    name,
                    has_rule,
                    min_uploaded,
                    min_download_count,
                    min_seed_hours,
                    min_account_age_days,
                    demotable,
                }
            },
        )
        .collect()
}

/// 职务级用户组（class_id >= 90）：只有名称，不列门槛。
pub async fn staff_classes(repo: &Repo) -> Vec<(i32, String)> {
    sqlx::query_as::<_, (i32, String)>(
        "SELECT id, name FROM user_classes WHERE id >= 90 ORDER BY id",
    )
    .fetch_all(&repo.read_db)
    .await
    .unwrap_or_default()
}

/// 本人当前等级 id。
pub async fn my_class(repo: &Repo, uid: i64) -> i32 {
    sqlx::query_scalar::<_, i32>("SELECT class_id FROM users WHERE id = $1")
        .bind(uid)
        .fetch_optional(&repo.read_db)
        .await
        .ok()
        .flatten()
        .unwrap_or(0)
}

/// 本人四维现状。
///
/// 下载数口径：snatches 里 completed_at 非空的条数（完成过的下载）；
/// 做种小时：snatches.seeded_seconds 合计 / 3600；
/// 账龄按日截断，与 class_auto_adjust 的升级判定一致。
pub async fn my_stats(repo: &Repo, uid: i64) -> Option<MeStats> {
    let row: Option<(i64, i64, i64, i64)> = sqlx::query_as(
        "SELECT u.uploaded::bigint, \
                (SELECT count(*) FROM snatches s WHERE s.user_id = u.id \
                  AND s.completed_at IS NOT NULL)::bigint, \
                (SELECT COALESCE(sum(s.seeded_seconds),0)/3600 FROM snatches s \
                  WHERE s.user_id = u.id)::bigint, \
                EXTRACT(DAY FROM now() - u.created_at)::bigint \
         FROM users u WHERE u.id = $1",
    )
    .bind(uid)
    .fetch_optional(&repo.read_db)
    .await
    .ok()
    .flatten();
    row.map(|(uploaded, download_count, seed_hours, account_age_days)| {
        MeStats {
            uploaded,
            download_count,
            seed_hours,
            account_age_days,
        }
    })
}

/// 本人发布数（名下种子数）。
///
/// 口径：torrents.owner_id = 本人，含未过审/被拒的 —— 与页面「发布数」
/// 同口径（发布过就算数），与「做种数」那种只算在种的分开。
pub async fn upload_count(repo: &Repo, uid: i64) -> i64 {
    sqlx::query_scalar::<_, i64>(
        "SELECT count(*)::bigint FROM torrents WHERE owner_id = $1",
    )
    .bind(uid)
    .fetch_optional(&repo.read_db)
    .await
    .ok()
    .flatten()
    .unwrap_or(0)
}

/// 本人魔力（火花）余额。
pub async fn spark_balance(repo: &Repo, uid: i64) -> i64 {
    sqlx::query_scalar::<_, i64>(
        "SELECT COALESCE(spark_balance, 0)::bigint FROM users WHERE id = $1",
    )
    .bind(uid)
    .fetch_optional(&repo.read_db)
    .await
    .ok()
    .flatten()
    .unwrap_or(0)
}
