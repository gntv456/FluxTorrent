//! 猜大小（/games/bigsmall）—— 从 games_http/casino.rs 按域拆出。
//!
//! 拆的动机不是洁癖：P-347 给奖金入账加「非 Spent 就整笔回滚」的断言时，
//! casino.rs 顶破了 300 行门禁的存量基线（只许瘦不许胖）。三件套共用的
//! 结算函数留在 casino.rs，本文件只放猜大小这一只口与它的道具加权。

use actix_web::{post, web, HttpRequest, HttpResponse};
use serde::Deserialize;

use crate::dto::ok;
use crate::economy_http::{earn_spark_tx, spend_spark_tx, SpendOutcome};
use crate::errors::{DomainError, DomainResult};
use crate::games::{self, Guess};
use crate::http::require_auth;
use crate::state::AppState;

use super::casino::{award_kind, meta_rarity, settle_tx};
use super::helpers::{check_bet, check_rate, idem_key};
use super::pool::{dberr, load_table};

#[derive(Deserialize)]
// 会员面「花火花」写口：多带的键不静默忽略（P-314 一族，普查见 scripts/pt_audit_write_unknown_field.py）。
// 猜大小：`props`（道具位）拼错时旧写法当没带道具——玩家的「用了放大镜」被吞，事后无从追查。
#[serde(deny_unknown_fields)]
struct GuessReq {
    bet: i64,
    guess: String, // "small" | "big"
    /// 客户端幂等键（审计 P2-8）
    #[serde(default)]
    idempotency_key: Option<String>,
    /// 本局挂载的道具 key（可选）：只加权魔力的输赢，永不出物品
    #[serde(default)]
    props: Vec<String>,
}

#[post("/games/bigsmall")]
pub(super) async fn guess_bigsmall(
    req: HttpRequest,
    state: web::Data<std::sync::Arc<AppState>>,
    body: web::Json<GuessReq>,
) -> DomainResult<HttpResponse> {
    let auth = require_auth(&req, &state).await?;
    let db = &state.repo.db;
    // 三区档位来自 arcade_pools(game='bigsmall')：**纯魔力**（物品位在读侧被拒），
    // 配坏则整桌拒绝服务（load_table 里过关）
    let table = load_table(db, "bigsmall").await?;
    check_bet(&state, body.bet)
        .await
        .map_err(DomainError::Validation)?;
    if body.bet < table.ticket {
        return Err(DomainError::Validation(format!(
            "最低注额为 {} 魔力",
            table.ticket
        )));
    }
    let guess = match body.guess.as_str() {
        "small" => Guess::Small,
        "big" => Guess::Big,
        _ => {
            return Err(DomainError::Validation(
                "guess 仅支持 small/big".into(),
            ))
        }
    };
    // 道具先校验（只读）再扣款：校验失败时不能已经扣了注额
    let props = super::bigsmall_props::pick(db, auth.id, &body.props).await?;
    check_rate(&state, &state.redis, auth.id).await?;

    let idem = idem_key("bs", auth.id, &body.idempotency_key);
    // 机制在代码（3 颗六面骰、105/6/105 分区），派彩在表。先定死结果再动账。
    let dice = games::roll();
    let side = games::outcome_side(dice, guess);
    let number: u32 = dice.iter().sum();
    let draw = games::draw_entry(table.region(side)).ok_or_else(|| {
        DomainError::Validation("猜大小该档区不可抽样".into())
    })?;
    let win_idem = format!("game-bs-win:{}", idem);
    let prop_idem = |k: &str| format!("game-bs-prop:{idem}:{k}");
    // 一笔事务走完：扣注额 → 扣道具 → 结算 → 道具加成。
    // 中间任何一步失败都整体回滚 —— 半途而废等于「注额和道具都扣了，什么都没发生」。
    let mut tx = state.repo.db.begin().await.map_err(dberr)?;
    // 幂等（同 scratch）：重放不重开，避免「首局没中 + 重放中了」白赚
    if !matches!(
        spend_spark_tx(
            &mut tx, auth.id, body.bet, "game", &idem, "bigsmall", 0
        )
        .await?,
        SpendOutcome::Spent
    ) {
        let _ = tx.rollback().await;
        return Err(DomainError::Validation("该局已受理，请勿重复提交".into()));
    }
    // 扣道具（幂等键 = 本局 + 道具 key）：重放不重复扣，也不吞掉一件
    for p in &props {
        super::bigsmall_props::consume_tx(
            &mut tx,
            auth.id,
            &p.key,
            "bigsmall",
            &prop_idem(&p.key),
        )
        .await?;
    }
    let (base_spark, value, fell_back) = settle_tx(
        &mut tx,
        auth.id,
        body.bet,
        &draw,
        "bigsmall",
        &win_idem,
        meta_rarity(table.meta(side), draw.index),
    )
    .await?;
    // 道具加权：只改魔力的输赢幅度，不产生任何物品
    let ap = super::bigsmall_props::apply(
        &props,
        body.bet,
        side,
        base_spark,
        draw.prize.mult_permille(),
    );
    if ap.extra > 0 {
        // 同 P-347：道具加成的那笔也要确认入账，回显 payout 之前不许沉默
        if !matches!(
            earn_spark_tx(
                &mut tx,
                auth.id,
                ap.extra,
                "game",
                &format!("game-bs-prop-win:{idem}"),
            )
            .await?,
            SpendOutcome::Spent
        ) {
            return Err(DomainError::LedgerConflict);
        }
    }
    tx.commit().await.map_err(dberr)?;
    let payout = base_spark + ap.extra;
    state.repo.audit(Some(auth.id), "game.bigsmall", None).await;
    Ok(ok(serde_json::json!({
        "number": number,
        "dice": dice,
        "player_win": side == "win",
        "side": side,
        "payout": payout,
        "base_payout": base_spark,
        "value": value + ap.extra,
        "prize": draw.prize.label,
        "kind": award_kind(&draw, fell_back),
        "fell_back": fell_back,
        "bet": body.bet,
        "net": payout - body.bet,
        "triple": side == "triple",
        "props_applied": props.iter().map(|p| serde_json::json!({
            "key": p.key, "name": p.name, "icon": p.icon,
            "effect": p.effect, "value": p.value,
        })).collect::<Vec<_>>(),
        "shield_refund": ap.shield_refund,
        "effective_mult": ap.effective_mult.map(|m| m as f64 / 1000.0),
    })))
}

// ============ 农场 ============

/// GET /games/bigsmall/trend —— 走势看板（0424，同类站对标小件）。
///
/// 近 N 局（默认 20，上限 50）的全站开奖序列 + 大/小/豹子分布占比。
/// 数据源 arcade_pool_rounds(game='bigsmall')：prize 列存区标签
/// （win/triple/lose），side 语义反推——lose 行无法从 prize 区分押注方向，
/// 但走势看板要的是「开奖落在哪区」，直接按 rarity/prize 聚合即可。
#[actix_web::get("/games/bigsmall/trend")]
pub(super) async fn bigsmall_trend(
    req: HttpRequest,
    state: web::Data<std::sync::Arc<AppState>>,
    q: web::Query<TrendQ>,
) -> DomainResult<HttpResponse> {
    let auth = require_auth(&req, &state).await?;
    let n = q.n.unwrap_or(20).clamp(1, 50);
    let rows: Vec<(String, i64, i32, chrono::DateTime<chrono::Utc>)> =
        sqlx::query_as(
            "SELECT prize, spark, rarity, created_at FROM arcade_pool_rounds \
             WHERE game = 'bigsmall' ORDER BY id DESC LIMIT $1",
        )
        .bind(n)
        .fetch_all(&state.repo.db)
        .await
        .map_err(dberr)?;
    // 区标签 → 开奖面（win = 命中所押面；走势只关心大/小/豹子三态，
    // 用 number 不可得（未落库），以 prize 近似：triple 标签 = 豹子；
    // 其余按 spark>0=win 行的两态展示为「开」）——分布口径在注释里说死。
    let mut seq: Vec<&str> = Vec::with_capacity(rows.len());
    let (mut big_n, mut small_n, mut triple_n) = (0i64, 0i64, 0i64);
    for (prize, _spark, _rarity, _at) in &rows {
        let face = if prize == "triple" {
            triple_n += 1;
            "triple"
        } else if *_spark > 0 {
            big_n += 1;
            "open"
        } else {
            small_n += 1;
            "blank"
        };
        seq.push(face);
    }
    let total = rows.len().max(1) as f64;
    state
        .repo
        .audit(Some(auth.id), "game.bigsmall_trend", None)
        .await;
    Ok(ok(serde_json::json!({
        "rounds": rows.iter().map(|(p, s, r, at)| serde_json::json!({
            "prize": p, "spark": s, "rarity": r, "at": at,
        })).collect::<Vec<_>>(),
        "seq": seq,
        "dist": {
            "win": (big_n as f64 / total * 100.0).round() / 100.0,
            "lose": (small_n as f64 / total * 100.0).round() / 100.0,
            "triple": (triple_n as f64 / total * 100.0).round() / 100.0,
        },
        "count": rows.len(),
    })))
}

#[derive(Deserialize)]
struct TrendQ {
    n: Option<i32>,
}
