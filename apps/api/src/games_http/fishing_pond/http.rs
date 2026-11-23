//! 甜梦钓场 HTTP 面：读数、抛竿、起竿、结算、通用动作、赛季榜。
//!
//! 所有写口都先锁住该用户那一行存档（`SELECT … FOR UPDATE`），在事务里改完再提交；
//! 抛竿/起竿/结算另挂在 `arcade_fishing_pond_rounds`，客户端只提交“结果”，
//! 鱼种、体型、价值与是否断线全部由服务端判定。

use actix_web::{get, post, web, HttpRequest, HttpResponse};
use serde::Deserialize;
use serde_json::{json, Value};

use crate::dto::ok;
use crate::economy_http::{earn_spark_tx, spend_spark_tx};
use crate::errors::{DomainError, DomainResult};
use crate::games_http::helpers::check_minclass;
use crate::http::require_auth;
use crate::state::AppState;

use super::data::data;
use super::rules;
use super::state::{
    self as st, now_ms, recover_stamina, season_ensure, season_info, today_key,
};

type Db = web::Data<std::sync::Arc<AppState>>;

fn internal(e: impl std::fmt::Display) -> DomainError {
    DomainError::Internal(anyhow::anyhow!(e.to_string()))
}

/// 站点魔力余额（与 users.spark_balance 同一行）。
async fn spark_of(
    tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    uid: i64,
) -> DomainResult<i64> {
    sqlx::query_scalar("SELECT spark_balance FROM users WHERE id = $1")
        .bind(uid)
        .fetch_one(&mut **tx)
        .await
        .map_err(|e| DomainError::Internal(e.into()))
}

/// 渔场只有一个货币：场内 `magic` 是站点余额的镜像。
/// 规则层照旧对 `st["magic"]` 加减，这里把本次动作的净差额落成一条 spark 流水：
/// 正差额入账、负差额扣款，然后把镜像改写成真实余额。
/// 规则层已经用镜像余额做过一次校验，正常不会出现“余额不足”；真撞上并发就整个事务回滚。
async fn mirror_magic(
    tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    uid: i64,
    before: i64,
    after: i64,
    idem_prefix: &str,
) -> DomainResult<i64> {
    let delta = after - before;
    if delta > 0 {
        let idem = format!("{}:{}:{}", idem_prefix, uid, uuid::Uuid::new_v4());
        let _ = earn_spark_tx(tx, uid, delta, "fishing_pond", &idem).await?;
    } else if delta < 0 {
        let idem = format!("{}:{}:{}", idem_prefix, uid, uuid::Uuid::new_v4());
        let _ =
            spend_spark_tx(tx, uid, -delta, "game", &idem, "fishing_pond", 0)
                .await?;
    }
    spark_of(tx, uid).await
}

async fn lock_state(
    tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    uid: i64,
) -> DomainResult<Value> {
    let now = now_ms();
    sqlx::query(
        "INSERT INTO arcade_fishing_pond (user_id, state) VALUES ($1, '{}') \
                 ON CONFLICT (user_id) DO NOTHING",
    )
    .bind(uid)
    .execute(&mut **tx)
    .await
    .map_err(internal)?;
    let raw: Option<Value> = sqlx::query_scalar(
        "SELECT state FROM arcade_fishing_pond WHERE user_id = $1 FOR UPDATE",
    )
    .bind(uid)
    .fetch_optional(&mut **tx)
    .await
    .map_err(internal)?;
    Ok(st::hydrate(
        raw.unwrap_or_else(|| st::default_state(now)),
        now,
    ))
}

async fn write_state(
    tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    uid: i64,
    state: &Value,
) -> DomainResult<()> {
    sqlx::query("UPDATE arcade_fishing_pond SET state = $2, updated_at = now() WHERE user_id = $1")
        .bind(uid)
        .bind(state)
        .execute(&mut **tx)
        .await
        .map_err(internal)?;
    Ok(())
}

/// 世界状态推进：体力恢复 / 当日订单 / 签到区 / 好友收件箱 / 赛季。
fn advance(st: &mut Value, now: i64) {
    recover_stamina(st, now);
    st::orders_ensure(st, now);
    st::friends_ensure(st, now);
    season_ensure(st, now);
}

fn state_view(st: &Value) -> Value {
    json!({"state": st, "server_ms": now_ms(), "today": today_key(now_ms())})
}

#[get("/games/fishing/pond")]
pub(super) async fn pond_state(
    req: HttpRequest,
    state: Db,
) -> DomainResult<HttpResponse> {
    let auth = require_auth(&req, &state).await?;
    check_minclass(&state, "fishing", auth.class_id).await?;
    let now = now_ms();
    let mut tx = state.repo.db.begin().await.map_err(internal)?;
    let mut st = lock_state(&mut tx, auth.id).await?;
    advance(&mut st, now);
    // 场内货币就是站点魔力：每次读数都对齐 users.spark_balance
    let spark = spark_of(&mut tx, auth.id).await?;
    st["magic"] = json!(spark);
    write_state(&mut tx, auth.id, &st).await?;
    tx.commit().await.map_err(internal)?;
    let mut view = state_view(&st);
    view["spark"] = json!(spark);
    Ok(ok(view))
}

/* ================= 抛竿 / 起竿 / 结算 ================= */

#[post("/games/fishing/pond/cast")]
pub(super) async fn pond_cast(
    req: HttpRequest,
    state: Db,
) -> DomainResult<HttpResponse> {
    let auth = require_auth(&req, &state).await?;
    check_minclass(&state, "fishing", auth.class_id).await?;
    let now = now_ms();
    let mut tx = state.repo.db.begin().await.map_err(internal)?;
    let mut st = lock_state(&mut tx, auth.id).await?;
    advance(&mut st, now);
    st["magic"] = json!(spark_of(&mut tx, auth.id).await?);
    let mut rng = rand::thread_rng();
    use rand::Rng as _;
    let mut r = move || rng.gen::<f64>();
    let cast = match rules::cast(&mut st, now, &mut r) {
        Ok(v) => v,
        Err(reason) => {
            let _ = tx.rollback().await;
            return Ok(ok(json!({"ok": false, "reason": reason, "state": st})));
        }
    };
    let fish = cast["fish"].clone();
    let stats = cast["stats"].clone();
    let bite_after = cast["bite_after_ms"].as_i64().unwrap_or(2400);
    let window_ms = cast["window_ms"].as_i64().unwrap_or(800);
    // 过期残局顺手清掉：不清理就只涨不消
    sqlx::query(
        "DELETE FROM arcade_fishing_pond_rounds \
          WHERE user_id = $1 AND resolved = false \
            AND created_at < now() - interval '1 hour'",
    )
    .bind(auth.id)
    .execute(&mut *tx)
    .await
    .map_err(internal)?;
    let round_id: i64 = sqlx::query_scalar(
        "INSERT INTO arcade_fishing_pond_rounds \
            (user_id, fish_id, rarity, size_cm, cast_stats, bite_after_ms, window_ms, bite_at) \
         VALUES ($1, $2, $3, $4, $5, $6, $7, now() + make_interval(secs => $8)) RETURNING id",
    )
    .bind(auth.id)
    .bind(type_of(&fish, "id"))
    .bind(type_of(&fish, "rarity"))
    .bind(cast["size"].as_f64().unwrap_or(1.0))
    .bind(&stats)
    .bind(bite_after)
    .bind(window_ms)
    .bind(bite_after as f64 / 1000.0)
    .fetch_one(&mut *tx)
    .await
    .map_err(internal)?;
    write_state(&mut tx, auth.id, &st).await?;
    tx.commit().await.map_err(internal)?;
    Ok(ok(json!({
        "ok": true, "round_id": round_id,
        "fish": fish, "size": cast["size"], "stats": stats,
        "bite_after_ms": bite_after, "window_ms": window_ms,
        "limit": cast["limit"], "pull": cast["pull"],
        "state": st
    })))
}

fn type_of(v: &Value, k: &str) -> String {
    v.get(k).and_then(Value::as_str).unwrap_or("").to_string()
}

#[derive(Deserialize)]
struct RoundReq {
    round_id: i64,
}

#[post("/games/fishing/pond/hook")]
pub(super) async fn pond_hook(
    req: HttpRequest,
    state: Db,
    body: web::Json<RoundReq>,
) -> DomainResult<HttpResponse> {
    let auth = require_auth(&req, &state).await?;
    check_minclass(&state, "fishing", auth.class_id).await?;
    let db = &state.repo.db;
    let row: Option<(i64, bool, chrono::DateTime<chrono::Utc>, i32)> =
        sqlx::query_as(
            "SELECT id, hooked_at IS NOT NULL, bite_at, window_ms \
           FROM arcade_fishing_pond_rounds WHERE id = $1 AND user_id = $2",
        )
        .bind(body.round_id)
        .bind(auth.id)
        .fetch_optional(db)
        .await
        .map_err(internal)?;
    let Some((_id, hooked, bite_at, window_ms)) = row else {
        return Ok(ok(json!({"ok": false, "reason": "missing"})));
    };
    let elapsed = now_ms() - bite_at.timestamp_millis();
    if hooked || elapsed < 0 || elapsed > i64::from(window_ms) {
        // 窗口外起竿：按跑鱼收尾，round 置终态
        sqlx::query(
            "UPDATE arcade_fishing_pond_rounds SET resolved = true, result = '{\"outcome\":\"late\"}' \
              WHERE id = $1 AND resolved = false",
        )
        .bind(body.round_id)
        .execute(db)
        .await
        .map_err(internal)?;
        return Ok(ok(json!({"ok": false, "reason": "late"})));
    }
    sqlx::query(
        "UPDATE arcade_fishing_pond_rounds SET hooked_at = now() WHERE id = $1",
    )
    .bind(body.round_id)
    .execute(db)
    .await
    .map_err(internal)?;
    Ok(ok(json!({"ok": true})))
}

#[derive(Deserialize)]
struct SettleReq {
    round_id: i64,
    outcome: String,
    #[serde(default)]
    duration_ms: i64,
}

#[post("/games/fishing/pond/settle")]
pub(super) async fn pond_settle(
    req: HttpRequest,
    state: Db,
    body: web::Json<SettleReq>,
) -> DomainResult<HttpResponse> {
    let auth = require_auth(&req, &state).await?;
    check_minclass(&state, "fishing", auth.class_id).await?;
    let now = now_ms();
    let mut tx = state.repo.db.begin().await.map_err(internal)?;
    let row: Option<(String, f64, Value, bool, bool, Option<chrono::DateTime<chrono::Utc>>)> = sqlx::query_as(
        "SELECT fish_id, size_cm::float8, cast_stats, resolved, hooked_at IS NOT NULL, hooked_at \
           FROM arcade_fishing_pond_rounds WHERE id = $1 AND user_id = $2 FOR UPDATE",
    )
    .bind(body.round_id)
    .bind(auth.id)
    .fetch_optional(&mut *tx)
    .await
    .map_err(internal)?;
    let Some((fish_id, size, stats, resolved, hooked, hooked_at)) = row else {
        return Ok(ok(json!({"ok": false, "reason": "missing"})));
    };
    if resolved {
        return Ok(ok(json!({"ok": false, "reason": "replayed"})));
    }
    let landing = body.outcome == "land";
    if landing && !hooked {
        return Ok(ok(json!({"ok": false, "reason": "nothooked"})));
    }
    if landing && (body.duration_ms < 300 || body.duration_ms > 26_000) {
        return Ok(ok(json!({"ok": false, "reason": "badtime"})));
    }
    // 反“秒收”作弊：客户端上报的收线时长必须与服务器观测到的
    // 起竿→结算真实间隔同量级（容 900ms 网络/帧抖动）。否则钩上立刻
    // 领奖就能跳过整个溜鱼过程。
    if landing {
        if let Some(h) = hooked_at {
            let elapsed = now - h.timestamp_millis();
            if body.duration_ms > elapsed + 900 {
                return Ok(ok(json!({"ok": false, "reason": "badtime"})));
            }
        } else {
            return Ok(ok(json!({"ok": false, "reason": "nothooked"})));
        }
    }
    let mut st = lock_state(&mut tx, auth.id).await?;
    advance(&mut st, now);
    // 场内货币 = 站点魔力镜像；本次结算的净收益落 spark 流水
    let magic_before = spark_of(&mut tx, auth.id).await?;
    st["magic"] = json!(magic_before);
    let mut rng = rand::thread_rng();
    use rand::Rng as _;
    let mut r = move || rng.gen::<f64>();
    let mut items: Vec<Value> = Vec::new();
    let mut outcome = body.outcome.as_str();
    if outcome == "land" {
        let Some(fish) = data().fish(&fish_id) else {
            let _ = tx.rollback().await;
            return Err(DomainError::Validation("鱼种已下架，请重抛".into()));
        };
        let stats_all = stats.get("all").and_then(Value::as_f64).unwrap_or(0.0);
        let s = rules::stats_from_cast(&stats, stats_all);
        items.push(rules::settle_catch(&mut st, fish, size, &s, now, &mut r));
        // 连竿：传说套 34%（其中 28% 双尾）／鱼竿 Lv≥4 16% 单尾
        let set_id = stats.get("setId").and_then(Value::as_str).unwrap_or("");
        let rod_lv = st::item_lv(&st, "rod", st::s(&st["equipped"], "rod"));
        let extra = if set_id == "dragonLegend" {
            if r() < 0.34 {
                if r() < 0.28 {
                    2
                } else {
                    1
                }
            } else {
                0
            }
        } else if rod_lv >= 4 && r() < 0.16 {
            1
        } else {
            0
        };
        for _ in 0..extra {
            let f = rules::roll_fish(&st, &mut r);
            let sz = rules::roll_size(f, &mut r);
            items.push(rules::settle_catch(&mut st, f, sz, &s, now, &mut r));
        }
        season_ensure(&mut st, now);
        st["season"]["catches"] = json!(st::n(&st["season"], "catches") + 1);
    } else {
        outcome = if body.outcome == "break" {
            "break"
        } else {
            "miss"
        };
        rules::settle_miss(&mut st, outcome);
    }
    let magic_after = st::n(&st, "magic");
    let balance = mirror_magic(
        &mut tx,
        auth.id,
        magic_before,
        magic_after,
        "pond_settle",
    )
    .await?;
    st["magic"] = json!(balance);
    sqlx::query(
        "UPDATE arcade_fishing_pond_rounds SET resolved = true, result = $2 \
          WHERE id = $1 AND resolved = false",
    )
    .bind(body.round_id)
    .bind(json!({"outcome": outcome, "duration_ms": body.duration_ms, "count": items.len()}))
    .execute(&mut *tx)
    .await
    .map_err(internal)?;
    write_state(&mut tx, auth.id, &st).await?;
    tx.commit().await.map_err(internal)?;
    Ok(ok(
        json!({"ok": true, "items": items, "state": st, "spark": balance}),
    ))
}

/* ================= 通用动作 ================= */

#[post("/games/fishing/pond/act")]
pub(super) async fn pond_act(
    req: HttpRequest,
    state: Db,
    body: web::Json<Value>,
) -> DomainResult<HttpResponse> {
    let auth = require_auth(&req, &state).await?;
    check_minclass(&state, "fishing", auth.class_id).await?;
    let now = now_ms();
    let action = body
        .get("a")
        .and_then(Value::as_str)
        .unwrap_or("")
        .to_string();
    let mut tx = state.repo.db.begin().await.map_err(internal)?;
    let mut st = lock_state(&mut tx, auth.id).await?;
    advance(&mut st, now);
    let magic_before = spark_of(&mut tx, auth.id).await?;
    st["magic"] = json!(magic_before);
    let r = apply_action(&mut st, &action, &body, now);
    match r {
        Ok(result) => {
            // 重置只清渔场进度，不能把站点余额重置成 3200（否则就是印钞）
            if action == "reset" {
                st["magic"] = json!(magic_before);
            }
            let magic_after = st::n(&st, "magic");
            let balance = mirror_magic(
                &mut tx,
                auth.id,
                magic_before,
                magic_after,
                "pond_act",
            )
            .await?;
            st["magic"] = json!(balance);
            write_state(&mut tx, auth.id, &st).await?;
            tx.commit().await.map_err(internal)?;
            Ok(ok(
                json!({"ok": true, "result": result, "state": st, "spark": balance}),
            ))
        }
        Err(reason) => {
            st["magic"] = json!(magic_before);
            let _ = tx.rollback().await;
            Ok(ok(json!({"ok": false, "reason": reason, "state": st})))
        }
    }
}

fn apply_action(
    st: &mut Value,
    a: &str,
    b: &Value,
    now: i64,
) -> Result<Value, &'static str> {
    let arg =
        |k: &str| b.get(k).and_then(Value::as_str).unwrap_or("").to_string();
    match a {
        "buy" => {
            let r = rules::buy_item(st, &arg("cat"), &arg("id"), now)?;
            let _ = rules::equip_item(st, &arg("cat"), &arg("id"));
            Ok(r)
        }
        "equip" => rules::equip_item(st, &arg("cat"), &arg("id")),
        "upgrade" => rules::upgrade_item(st, &arg("cat"), &arg("id")),
        "buyScene" => rules::buy_scene(st, &arg("id")),
        "switchScene" => rules::switch_scene(st, &arg("id")),
        "cond" => rules::set_cond(st, &arg("c")),
        "expandBag" => rules::expand_bag(st),
        "unlockAuto" => rules::unlock_auto(st),
        "refill" => rules::refill_stamina(st, now),
        "claimOrder" => rules::claim_order(
            st,
            b.get("idx").and_then(Value::as_u64).unwrap_or(0) as usize,
        ),
        "claimOrderBonus" => rules::claim_order_bonus(st),
        "claimAch" => rules::claim_ach(st, &arg("id"), now),
        "releaseBagFish" => rules::release_from_bag(st, &arg("uid")),
        "takeFish" => rules::take_from_aquarium(st, &arg("uid")),
        "releaseFish" => rules::release_from_aquarium(st, &arg("uid")),
        "takeAll" => Ok(rules::take_all_from_aquarium(st)),
        "releaseAll" => {
            let n = st["aquarium"].as_array().map_or(0, |a| a.len());
            st["aquarium"] = json!([]);
            Ok(json!({"ok": true, "moved": 0, "released": n}))
        }
        "sign" => {
            let (rw, idx) = st::sign_claim(st, now)?;
            Ok(json!({"ok": true, "reward": rw, "idx": idx}))
        }
        "gift" => {
            let left = st::friends_gift(st)?;
            Ok(json!({"ok": true, "left": left}))
        }
        "receiveGifts" => {
            let count = st::friends_receive(st)?;
            Ok(json!({"ok": true, "count": count}))
        }
        "equipSet" => {
            let id = arg("id");
            let Some(set) = data().sets.iter().find(|s| s.id == id) else {
                return Err("notfound");
            };
            if !st["scenesOwned"].as_array().map_or(false, |a| {
                a.iter().any(|x| x.as_str() == Some(set.scene.as_str()))
            }) {
                return Err("locked");
            }
            st["scene"] = json!(set.scene);
            for (cat, item) in &set.parts {
                if st::is_owned(st, cat, item) {
                    st["equipped"][cat] = json!(item);
                }
            }
            Ok(json!({"ok": true, "setId": set.id}))
        }
        "gotoScene" => {
            rules::switch_scene(st, &arg("id"))?;
            rules::set_cond(st, &arg("cond"))?;
            Ok(json!({"ok": true}))
        }
        "reset" => {
            *st = st::default_state(now);
            st::ensure_equipped(st, now);
            Ok(json!({"ok": true}))
        }
        "setAuto" => {
            st["auto"] =
                json!(b.get("on").and_then(Value::as_bool).unwrap_or(false));
            Ok(json!({"ok": true}))
        }
        _ => Err("unknown"),
    }
}

/* ================= 赛季榜 ================= */

#[get("/games/fishing/pond/rank")]
pub(super) async fn pond_rank(
    req: HttpRequest,
    state: Db,
) -> DomainResult<HttpResponse> {
    let auth = require_auth(&req, &state).await?;
    check_minclass(&state, "fishing", auth.class_id).await?;
    let now = now_ms();
    let (id, day_idx, left) = season_info(now);
    let rows: Vec<(i64, i64, String)> = sqlx::query_as(
        "SELECT p.user_id, \
                COALESCE((p.state->'season'->>'catches')::bigint, 0) AS catches, \
                u.username \
           FROM arcade_fishing_pond p JOIN users u ON u.id = p.user_id \
          WHERE COALESCE((p.state->'season'->>'id')::bigint, 0) = $1 \
          ORDER BY catches DESC, p.user_id ASC LIMIT 50",
    )
    .bind(id)
    .fetch_all(&state.repo.db)
    .await
    .map_err(internal)?;
    let me: i64 = sqlx::query_scalar(
        "SELECT COALESCE((state->'season'->>'catches')::bigint, 0) \
           FROM arcade_fishing_pond WHERE user_id = $1",
    )
    .bind(auth.id)
    .fetch_optional(&state.repo.db)
    .await
    .map_err(internal)?
    .unwrap_or(0);
    let tones = ["sky", "sun", "lilac", "deep", "mint"];
    let mut out: Vec<Value> = Vec::new();
    for (i, (uid, catches, name)) in rows.iter().enumerate() {
        out.push(json!({
            "id": uid.to_string(), "name": name, "tag": "玩家",
            "tone": tones[i % tones.len()], "catches": catches,
            "reward": data().rank_rewards.get(i).copied().unwrap_or(50),
            "rank": i + 1, "me": *uid == auth.id
        }));
    }
    if !out.iter().any(|r| r["me"].as_bool().unwrap_or(false)) {
        out.push(json!({
            "id": "me", "name": "我的渔场", "tag": "我", "tone": "deep",
            "catches": me, "reward": 0, "rank": out.len() + 1, "me": true
        }));
    }
    let my_rank = out
        .iter()
        .find(|r| r["me"].as_bool().unwrap_or(false))
        .cloned();
    Ok(ok(json!({
        "season": {"id": id, "ordinal": id + 1, "dayIndex": day_idx, "leftDays": left},
        "rows": out, "me": my_rank.unwrap_or_else(|| json!({"catches": me, "rank": 0})),
        "seasonCatches": me
    })))
}
