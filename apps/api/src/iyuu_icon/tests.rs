//! 图标代理的类型判定单测。SSRF 闸复用 `domain::webhook::egress_allowed`
//! （那边有完整用例），体积/超时/重定向只能在运行期验，这里只管
//! 类型白名单与 mime 归一。

use super::{is_image, normalize_ct};

/// 图片类型放行。
#[test]
fn accepts_image_types() {
    assert!(is_image("image/x-icon"));
    assert!(is_image("image/png"));
    assert!(is_image("IMAGE/VND.MICROSOFT.ICON"));
    assert!(is_image(" image/svg+xml "));
}

/// 很多站的 .ico 发成 octet-stream，或干脆不给类型 —— 都要放行，
/// 否则一半站点没图标。
#[test]
fn accepts_octet_stream_and_empty() {
    assert!(is_image("application/octet-stream"));
    assert!(is_image(""));
}

/// 明显不是图的一律拒：对方把首页/错误页当 favicon 返回时不要存。
#[test]
fn rejects_non_image_types() {
    assert!(!is_image("text/html"));
    assert!(!is_image("application/json"));
    assert!(!is_image("text/plain; charset=utf-8"));
}

/// 放行过的类型必须能变成非空 mime：空串直接当 content-type 会让
/// 浏览器把 .ico 当文本，图上不显示。
#[test]
fn normalize_ct_never_returns_empty() {
    assert_eq!(normalize_ct(""), "image/x-icon");
    assert_eq!(normalize_ct("application/octet-stream"), "image/x-icon");
    assert_eq!(normalize_ct("IMAGE/PNG"), "image/png");
    assert!(!normalize_ct("").is_empty());
}
