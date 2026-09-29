//! 票根册：复用 achievement_defs(family='arcade') + user_achievements，不新建收集表。
//! metric 从 spark_ledger(kind='game') 实时聚合 —— worker 的 achievement_grant 只认
//! 5 个老 metric（seeding_bytes/post_count/…），不为它加 game 分支（那会让 3 处重复
//! SQL 再膨胀一圈）；达标即幂等落 user_achievements，holders 计数天然来自该表。

use sqlx::PgPool;

#[derive(sqlx::FromRow)]
pub(super) struct StubRow {
    pub(super) id: i64,
    pub(super) code: String,
    pub(super) name: String,
    pub(super) descr: String,
    pub(super) metric: String,
    pub(super) threshold: i64,
    pub(super) position: i32,
    pub(super) obtained: bool,
}

async fn grant(
    db: &PgPool,
    uid: i64,
    def_id: i64,
    val: i64,
) -> Result<(), sqlx::Error> {
    sqlx::query(
        "INSERT INTO user_achievements (user_id, def_id, metric_value) \
         VALUES ($1, $2, $3) ON CONFLICT (user_id, def_id) DO NOTHING",
    )
    .bind(uid)
    .bind(def_id)
    .bind(val)
    .execute(db)
    .await?;
    Ok(())
}

/// 同步并返回票根册（JSON 列表 + 已集齐数）。
pub(super) async fn sync_stubs(
    db: &PgPool,
    uid: i64,
) -> Result<(Vec<serde_json::Value>, i64), sqlx::Error> {
    let mut stubs: Vec<StubRow> = sqlx::query_as(
        "SELECT d.id, d.code, d.name, d.descr, d.metric, d.threshold, \
                d.position, (ua.user_id IS NOT NULL) AS obtained \
         FROM achievement_defs d \
         LEFT JOIN user_achievements ua \
           ON ua.def_id = d.id AND ua.user_id = $1 \
         WHERE d.family = 'arcade' ORDER BY d.position",
    )
    .bind(uid)
    .fetch_all(db)
    .await?;
    let m: (i64, i64, i64, i64, i64, i64) = sqlx::query_as(
        "SELECT count(*)::bigint, \
                count(*) FILTER (WHERE ref_type = 'scratch')::bigint, \
                count(*) FILTER (WHERE ref_type = 'jgg')::bigint, \
                count(*) FILTER (WHERE ref_type = 'bigsmall')::bigint, \
                count(*) FILTER (WHERE ref_type = 'farm_water')::bigint, \
                count(*) FILTER (WHERE ref_type = 'farm_plant')::bigint \
         FROM spark_ledger \
         WHERE user_id = $1 AND kind = 'game' AND amount < 0",
    )
    .bind(uid)
    .fetch_one(db)
    .await?;
    let metric_of = |k: &str| -> Option<i64> {
        match k {
            "game_plays" => Some(m.0),
            "scratch_plays" => Some(m.1),
            "jgg_plays" => Some(m.2),
            "bs_plays" => Some(m.3),
            "farm_water" => Some(m.4),
            "farm_plant" => Some(m.5),
            _ => None,
        }
    };
    // 第一轮：非「集齐类」票根
    for s in stubs.iter_mut().filter(|s| s.metric != "stub_count") {
        if !s.obtained {
            if let Some(v) = metric_of(&s.metric) {
                if v >= s.threshold {
                    grant(db, uid, s.id, v).await?;
                    s.obtained = true;
                }
            }
        }
    }
    // 第二轮：集齐类（依赖上一轮结果）
    let mut owned = stubs.iter().filter(|s| s.obtained).count() as i64;
    for s in stubs.iter_mut().filter(|s| s.metric == "stub_count") {
        if !s.obtained && owned >= s.threshold {
            grant(db, uid, s.id, owned).await?;
            s.obtained = true;
            owned += 1;
        }
    }
    let ids: Vec<i64> = stubs.iter().map(|s| s.id).collect();
    let holders: Vec<(i64, i64)> = sqlx::query_as(
        "SELECT def_id, count(*)::bigint FROM user_achievements \
         WHERE def_id = ANY($1) GROUP BY def_id",
    )
    .bind(&ids)
    .fetch_all(db)
    .await?;
    let got = stubs.iter().filter(|s| s.obtained).count() as i64;
    let list = stubs
        .iter()
        .map(|s| {
            let h = holders
                .iter()
                .find(|x| x.0 == s.id)
                .map(|x| x.1)
                .unwrap_or(0);
            serde_json::json!({
                "code": s.code, "name": s.name, "descr": s.descr,
                "obtained": s.obtained, "holders": h, "position": s.position,
            })
        })
        .collect();
    Ok((list, got))
}
