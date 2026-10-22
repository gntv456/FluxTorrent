//! 公开主页的「隐私等级」执行边界（批八十三 P-293）。
//!
//! `usercp-security.tsx` 三档单选是给成员的承诺，库里 `users.privacy`
//! 也确实存着（写口还做了枚举校验）：
//!   normal 普通 / low 低(邮箱地址公开) / strong 强(个人信息全部隐藏)。
//! 此前它只有两个读者——写口校验与**自己**的档案回显（GET /me/settings），
//! `GET /users/{id}` 把任何人的个人信息原样发给任何登录成员：
//! 性别 / 简介 / 签名 / 上行带宽 / 下行带宽 / BT 客户端 / 邀请人。
//! 即「存了 + 回显了，但没有任何执行路径读它」那一族的隐私版。
//!
//! 为什么只接 `strong`：`low` 的括注讲的是**邮箱**，而 `/users/{id}` 与
//! `/users/{id}/torrentlist` 都不下发邮箱（`inviter` 那一条的注释就是当年
//! 刻意脱敏的结果），这一档在本接口**没有可执行的语义**。按既有纪律：
//! 语义没对上的档宁可不接，由闸门钉桩 + 文档记「面板文案与实现不符」，
//! 也不硬套最近的一档做假收口。

use actix_web::web;

use crate::authz;
use crate::state::AppState;

/// 是否要按本人的隐私等级遮蔽：观看者**不是本人**、也没有 `user.view`
/// （职员看他人档案用的就是这把已有键，不新增权限键）时才继续判定。
pub(super) async fn hides_personal(
    state: &web::Data<std::sync::Arc<AppState>>,
    auth: &crate::http::AuthUser,
    target: i64,
) -> bool {
    if auth.id == target {
        return false;
    }
    if authz::can(state, auth, authz::perm::USER_VIEW).await {
        return false;
    }
    let privacy =
        crate::repo::user_privacy::privacy_level(&state.repo, target).await;
    personal_hidden(privacy.as_deref())
}

/// 纯判定（单测钉住）：只有 `strong` 隐藏；`normal`/`low`/NULL/脏值都按
/// 「不隐藏」——脏值不猜意思，宁可与修前一致也不静默收窄视图。
pub(super) fn personal_hidden(privacy: Option<&str>) -> bool {
    privacy.unwrap_or("normal") == "strong"
}

/// 面板说的「个人信息」在这一屏上就是这些键（新增个人信息列要一起加；
/// DA 闸门按这份清单逐键断言，键名写错=漏遮，判据会红）。
pub(super) const PERSONAL_KEYS: [&str; 9] = [
    "gender",
    "info",
    "signature",
    "upload_speed",
    "download_speed",
    "client_agent",
    "inviter_id",
    "inviter_name",
    "user_fields",
];

/// 公开主页响应的隐私遮蔽入口：**判定 + 遮蔽合成一步**，让承载文件
/// `profile.rs`（297/300 行）只多得起一行调用。
///
/// 为什么按**键**在响应体上做，而不是逐个 `let` 改写：逐字段改写要 15 行，
/// 顶破行数门禁；键清单集中成本模块的 `PERSONAL_KEYS`，DA 闸门按同一份
/// 清单逐键断言（写漏一个键就红）。不隐藏的那条路径逐字节不变（早退）。
pub(super) async fn mask_personal(
    state: &web::Data<std::sync::Arc<AppState>>,
    auth: &crate::http::AuthUser,
    target: i64,
    body: &mut serde_json::Value,
) {
    if !hides_personal(state, auth, target).await {
        return;
    }
    apply(body, true);
}

/// 纯遮蔽动作（单测直接打这一层，不用起库）
fn apply(body: &mut serde_json::Value, hide: bool) {
    if !hide {
        return;
    }
    let Some(map) = body.as_object_mut() else {
        return;
    };
    for key in PERSONAL_KEYS {
        if !map.contains_key(key) {
            continue;
        }
        let blank = if key == "user_fields" {
            serde_json::Value::Array(Vec::new())
        } else {
            serde_json::Value::Null
        };
        map.insert(key.to_string(), blank);
    }
    // P-573：`profile` 子对象里还序列化了一份 signature（PublicProfile），
    // 只清顶层时强隐私档在 data.profile.signature 照旧把签名档发出去。
    if let Some(p) = map.get_mut("profile").and_then(|p| p.as_object_mut()) {
        if p.contains_key("signature") {
            p.insert("signature".into(), serde_json::Value::Null);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{apply, personal_hidden};

    #[test]
    fn only_strong_hides_personal_info() {
        assert!(personal_hidden(Some("strong")), "强=个人信息全部隐藏");
        assert!(!personal_hidden(Some("low")), "低只管邮箱，本接口无邮箱");
        assert!(!personal_hidden(Some("normal")), "普通=不隐藏");
        assert!(!personal_hidden(None), "NULL 按普通");
        assert!(!personal_hidden(Some("Strong")), "脏值不猜");
    }

    /// 遮蔽必须只动清单里的键，且 `hide=false` 时逐字节不变
    #[test]
    fn mask_only_touches_listed_keys() {
        let mut body = serde_json::json!({
            "gender": "男", "info": "简介", "signature": "签名",
            "upload_speed": 100, "download_speed": 20,
            "client_agent": "qBittorrent/4.6", "inviter_id": 7,
            "inviter_name": "someone", "user_fields": [{"k": 1}],
            "username": "tom", "spark_balance": 12,
            "profile": {"id": 7, "username": "tom", "signature": "签名"},
        });
        let before = body.clone();
        apply(&mut body, false);
        assert_eq!(body, before, "不隐藏时不许动一个字节");
        apply(&mut body, true);
        for key in super::PERSONAL_KEYS {
            let v = &body[key];
            if key == "user_fields" {
                assert_eq!(v.as_array().map(Vec::len), Some(0), "{key}");
            } else {
                assert!(v.is_null(), "{key} 必须 null");
            }
        }
        // P-573：嵌套 profile 里那份签名也是同一份个人信息
        assert!(body["profile"]["signature"].is_null(), "profile.signature");
        assert_eq!(body["profile"]["username"], "tom", "嵌套公开列不许误伤");
        assert_eq!(body["username"], "tom", "公开身份列不许被误伤");
        assert_eq!(body["spark_balance"], 12, "统计列不许被误伤");
    }
}
