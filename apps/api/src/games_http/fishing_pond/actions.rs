//! 钓场通用动作分发（从 http.rs 拆出：撞 300 行门禁）。

use serde_json::{json, Value};

use super::bait;
use super::data::data;
use super::rules;
use super::state as st;

pub(super) fn apply_action(
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
        // 打窝饵（每竿消耗 1 枚；高档饵用魔力买 —— 主回收口）
        "buyBait" => bait::buy_bait(
            st,
            &arg("id"),
            b.get("qty").and_then(Value::as_i64).unwrap_or(1),
        ),
        "selectBait" => bait::select_bait(st, &arg("id")),
        // 放生得渔运（只计点数不碰魔力）；渔运兑换解锁
        "release" => bait::release_for_luck(st, &arg("from"), &arg("uid")),
        "luckBuyScene" => bait::luck_buy_scene(st, &arg("id")),
        "luckBuyItem" => bait::luck_buy_item(st, &arg("cat"), &arg("id")),
        "luckExpandBag" => bait::luck_expand_bag(st),
        "luckUnlockAuto" => bait::luck_unlock_auto(st),
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
            let (rw, idx) = super::sign::sign_claim(st, now)?;
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
            // 重置只清渔场进度：`default_state` 的 magic 是 0，不还原成
            // 动作前余额的话，外层 `mirror_magic` 会把玩家**全部余额**当
            // 成一次消费扣光（before − 0）。余额不属于「渔场进度」。
            let keep = st::n(st, "magic");
            *st = st::default_state(now);
            st["magic"] = json!(keep);
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
