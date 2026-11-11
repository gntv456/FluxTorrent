//! 协议解析单测。两个 fixture 都是 2026-11-11 用真凭据打 IYUU 接口
//! 抓回来的原始响应片段，不是凭想象造的样本。

use serde_json::json;

use super::{is_hex40, parse_hits, parse_sites, sign, Hit, Site};

/// 真机查重响应：`data[0].torrent` 才是命中数组。
#[test]
fn hits_real_response() {
    let raw = include_str!("fixture_real.json");
    let p: serde_json::Value = serde_json::from_str(raw).unwrap();
    let got = parse_hits(&p);
    assert_eq!(got.len(), 6);
    assert_eq!(
        got.iter().map(|h| h.sid).collect::<Vec<_>>(),
        vec![1, 2, 3, 6, 9, 15]
    );
    assert_eq!(
        got[0],
        Hit {
            sid: 1,
            torrent_id: 6747
        }
    );
}

/// 同一个站在别处有多个同种时只保留一条（面板一行一个站）。
#[test]
fn hits_dedups_by_site() {
    let p = json!({
        "data": [{"torrent": [
            {"sid": 3, "torrent_id": 100},
            {"sid": 3, "torrent_id": 200},
            {"sid": 9, "torrent_id": 300}
        ]}]
    });
    let got = parse_hits(&p);
    assert_eq!(got.len(), 2);
    assert_eq!(
        got[0],
        Hit {
            sid: 3,
            torrent_id: 100
        }
    );
    assert_eq!(
        got[1],
        Hit {
            sid: 9,
            torrent_id: 300
        }
    );
}

/// 缺 sid / torrent_id 的行丢弃；畸形输入不 panic。
#[test]
fn hits_tolerates_garbage() {
    let incomplete = json!({"data": [{"torrent": [
        {"sid": 1}, {"torrent_id": 2}
    ]}]});
    assert!(parse_hits(&incomplete).is_empty());
    assert!(parse_hits(&json!("boom")).is_empty());
    assert!(parse_hits(&json!({})).is_empty());
    let bad = json!({"data": [{"torrent": "x"}]});
    assert!(parse_hits(&bad).is_empty());
}

/// 真机站点表：125 站，带详情页模板。
#[test]
fn sites_real_response() {
    let raw = include_str!("fixture_sites.json");
    let p: serde_json::Value = serde_json::from_str(raw).unwrap();
    let m = parse_sites(&p);
    assert_eq!(m.len(), 5);
    let ours = &m[&137];
    assert_eq!(ours.name, "好学");
    assert_eq!(ours.host, "www.hxpt.org");
    assert_eq!(ours.details, "details.php?id={}");
    // is_https=2 也是 https
    assert!(m[&3].https);
    assert_eq!(m[&3].details, "detail/{}");
}

/// 站点表异常形态一律空表。
#[test]
fn sites_tolerates_garbage() {
    assert!(parse_sites(&json!({"code": 404})).is_empty());
    assert!(parse_sites(&json!({"code": 200})).is_empty());
    let no_sites = json!({"code": 200, "data": {"count": 0}});
    assert!(parse_sites(&no_sites).is_empty());
    let bad = json!({"code": 200, "data": {"sites": "x"}});
    assert!(parse_sites(&bad).is_empty());
}

/// 详情页：按模板拼，M-Team 换 host，缺模板不给链接。
#[test]
fn detail_url_uses_template() {
    let np = Site::plain("天空", "hdsky.me");
    assert_eq!(
        np.detail_url(123).as_deref(),
        Some("https://hdsky.me/details.php?id=123")
    );
    let mteam = Site {
        name: "馒头".into(),
        host: "api.m-team.cc".into(),
        details: "detail/{}".into(),
        https: true,
    };
    assert_eq!(
        mteam.detail_url(9).as_deref(),
        Some("https://kp.m-team.cc/detail/9")
    );
    let gazelle = Site {
        name: "海豚".into(),
        host: "dicmusic.com".into(),
        details: "torrents.php?id={}".into(),
        https: true,
    };
    assert_eq!(
        gazelle.detail_url(7).as_deref(),
        Some("https://dicmusic.com/torrents.php?id=7")
    );
    let no_tpl = Site {
        name: "x".into(),
        host: "x.example".into(),
        details: String::new(),
        https: true,
    };
    assert!(no_tpl.detail_url(1).is_none());
    let http_only = Site {
        name: "y".into(),
        host: "y.example".into(),
        details: "details.php?id={}".into(),
        https: false,
    };
    assert_eq!(
        http_only.detail_url(2).as_deref(),
        Some("http://y.example/details.php?id=2")
    );
}

/// 签名 = sha1(timestamp + secret)，与旧站 PHP 逐字节一致。
#[test]
fn sign_matches_legacy_php() {
    let expect = "62590f31dce93245f2fe7e821ac3bab9a6ef85ff";
    assert_eq!(sign("secret", 1_700_000_000), expect);
}

/// 指纹形状校验。
#[test]
fn hex40_guard() {
    assert!(is_hex40("38f5e93b017ea9886183d1f3440084c29d03f502"));
    assert!(!is_hex40("38f5"));
    assert!(!is_hex40("zzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzz"));
}
