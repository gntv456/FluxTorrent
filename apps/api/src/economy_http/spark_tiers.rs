//! 我的魔力页「做种收益计算」系数表（从 voucher_use.rs 按域拆出，300 行门禁）。
//! 参数取自 seeding_params() + 0488 的 seeding_official_mult()，
//! 与 worker 结算共用同一真相源；本文件只构造展示用 JSON，不参与计费。

/// 完整公式人话版（前端 /my-spark 计算页展示）。
/// 用 concat! 分段，避免单行超 80 列门禁。
pub(super) const FORMULA_NOTE: &str = concat!(
    "每小时 = 底薪 + (2/π)·封顶·atan(Σ加成 × 曲线系数)，捐赠者整笔翻倍，",
    "再乘等级加成（1 + 等级比率）；",
    "每颗种子加成 =（种子档位分 + 做种时长档位分）× 体积因子 × 稀有度加成",
    " × 标定系数 × 官种倍率（仅官种生效，非官种 1.0）",
    " × 官区加成（官种 ×(1+10%)）；",
    "种子档位：濒危保种 2.0 / 高龄种 1.5 / 老种 1.0 / 大体积 0.75",
    " / 中体积 0.5 / 日常 0.25；",
    "做种时长档位：≥1年 +2.0 / 6-12月 +1.0 / 3-6月 +0.75 / 1-3月 +0.5；",
    "体积按对数饱和（≥50GB 记满分，1GB 约 0.18）；",
    "稀有度独苗 ×1.6、人多趋近 ×1.0；",
    "官种额外加成仍进 Σ 与 arctan 封顶，不叠加独立收益。",
    "另有流量积分独立入账：每上传 1GB +0.5、每下载 1GB +0.1（可调）。",
);

/// 结构化系数表：站长可调参数 + 档位阈值（阈值与 worker 结算 CASE 同源）。
/// 0525：追加等级加成（按调用者 class 取率）、官区加成、流量积分两费率。
pub(super) async fn build_tiers(
    db: &sqlx::PgPool,
    user_id: i64,
) -> serde_json::Value {
    let (
        base,
        cap,
        curve_k,
        vol_base,
        rarity_k,
        scale,
        donor_mult,
        official_mult,
        area_official_rate,
        traffic_up_rate,
        traffic_down_rate,
        class_id,
        class_rate,
    ): (
        f64,
        f64,
        f64,
        f64,
        f64,
        f64,
        f64,
        f64,
        f64,
        f64,
        f64,
        i32,
        f64,
    ) = sqlx::query_as(
        "SELECT base::float8, cap::float8, curve_k::float8, \
                    vol_base::float8, rarity_k::float8, scale::float8, \
                    donor_mult::float8, seeding_official_mult()::float8, \
                    seeding_area_official_rate()::float8, \
                    COALESCE((SELECT value::float8 FROM site_settings \
                              WHERE name='seeding_traffic_up_rate'), 0.5), \
                    COALESCE((SELECT value::float8 FROM site_settings \
                              WHERE name='seeding_traffic_down_rate'), 0.1), \
                    u.class_id, seeding_class_rate(u.class_id)::float8 \
             FROM seeding_params(), \
             (SELECT class_id FROM users WHERE id = $1) u",
    )
    .bind(user_id)
    .fetch_one(db)
    .await
    .unwrap_or((
        10.0, 150.0, 0.12, 50.0, 0.6, 0.4, 2.0, 1.5, 0.10, 0.5, 0.1, 0, 0.0,
    ));
    serde_json::json!({
        "formula": {
            "base_per_hour": base,
            "cap_per_hour": cap,
            "curve_k": curve_k,
            "scale": scale,
            "donor_mult": donor_mult,
            "official_mult": official_mult,
            "note": "每小时 = 底薪 + (2/π)·封顶·atan(Σ加成 × 曲线系数)，\
                     捐赠者整笔翻倍，再乘等级加成",
        },
        "official": {
            "mult": official_mult,
            "note": "官种（official_tag）单种加成额外乘该倍率；\
                     1.0 = 关闭。仍进 Σ → arctan 封顶，不叠加独立收益。",
        },
        "area": {
            "official_rate": area_official_rate,
            "note": "官区加成：官种单种积分额外 ×(1+比率)；\
                     与官种倍率并存（数值 × 区域两层口径）。",
        },
        "class_bonus": {
            "class_id": class_id,
            "rate": class_rate,
            "note": "等级加成：做种时薪 ×(1+等级比率)，等级越高越厚。",
        },
        "traffic": {
            "up_rate_per_gb": traffic_up_rate,
            "down_rate_per_gb": traffic_down_rate,
            "note": "流量积分独立入账：每上传/下载 1GB 按费率计积分，\
                     不进做种封顶曲线。",
        },
        "torrent_tier": [
            { "name": "濒危保种", "score": 2.0, "cond": "做种者≤1 且 完成≥3" },
            { "name": "高龄种",   "score": 1.5, "cond": "种龄 > 365 天" },
            { "name": "老种",     "score": 1.0, "cond": "种龄 > 180 天" },
            { "name": "大体积种", "score": 0.75, "cond": "体积 ≥ 100GB" },
            { "name": "中体积种", "score": 0.5,  "cond": "体积 ≥ 25GB" },
            { "name": "日常种",   "score": 0.25, "cond": "其余" },
        ],
        "seed_hours_tier": [
            { "name": "≥1年",  "score": 2.0 },
            { "name": "6-12月", "score": 1.0 },
            { "name": "3-6月", "score": 0.75 },
            { "name": "1-3月", "score": 0.5 },
        ],
        "volume": {
            "vol_base": vol_base,
            "note": "对数饱和：≥50GB 记满分，1GB 约 0.18",
        },
        "rarity": {
            "k": rarity_k,
            "note": "独苗 ×1.6、人多趋近 ×1.0",
        },
    })
}
