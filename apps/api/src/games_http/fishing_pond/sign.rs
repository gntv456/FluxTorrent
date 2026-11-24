//! 钓场签到（从 state.rs 拆出：撞 300 行门禁）。

//! 回收口径（2026-11-24）：签到只发竿数/体力/鱼饵，不发魔力 ——
//! 原魔力日 / 传说饵箱的额度折成免费竿（`sign_casts`）。

use serde_json::{json, Value};

use super::data::data;
use super::state::{n, s, today_key};

pub fn sign_claim(
    st: &mut Value,
    now: i64,
) -> Result<(Value, i64), &'static str> {
    let key = today_key(now);
    if s(&st["sign"], "last") == key {
        return Err("claimed");
    }
    let rewards = &data().k.sign_in_rewards;
    let idx = (n(&st["sign"], "idx") % rewards.len() as i64) as usize;
    let rw = &rewards[idx];
    st["sign"]["last"] = json!(key);
    st["sign"]["idx"] = json!(((idx + 1) as i64) % rewards.len() as i64);
    if (idx + 1) == rewards.len() {
        st["sign"]["cycle"] = json!(n(&st["sign"], "cycle") + 1);
        st["stats"]["signFull"] = json!(n(&st["stats"], "signFull") + 1);
    }
    // 签到只发「竿数」，不发魔力（钓场是回收口）：魔力日 / 饵箱的额度
    // 折成免费竿（`sign_casts`），折竿后按「鱼饵」类型回报。
    let free = match rw.kind.as_str() {
        "magic" => sign_casts(rw.value),
        "chest" => 4 + sign_casts(rw.value),
        "stamina" => 0,
        "bait" => rw.value,
        _ => 0,
    };
    if matches!(rw.kind.as_str(), "magic" | "chest") {
        st["stats"]["magicEarned"] =
            json!(n(&st["stats"], "magicEarned") + rw.value);
    }
    if rw.kind == "stamina" {
        st["stamina"]["v"] = json!(
            (n(&st["stamina"], "v") + rw.value).min(data().k.stamina_max)
        );
    }
    if free > 0 {
        st["freeCasts"] = json!(n(st, "freeCasts") + free);
    }
    let ty = if free > 0 && rw.kind != "bait" {
        "bait"
    } else {
        rw.kind.as_str()
    };
    let val = if free > 0 { free } else { rw.value };
    Ok((
        json!({ "type": ty, "value": val, "label": rw.label }),
        idx as i64,
    ))
}

/// 原魔力奖励折成免费竿数：⌈魔力 ÷ 10 × 3⌉，至少 1 竿。
fn sign_casts(magic: i64) -> i64 {
    ((magic.max(1) * 3 + 9) / 10).max(1)
}
