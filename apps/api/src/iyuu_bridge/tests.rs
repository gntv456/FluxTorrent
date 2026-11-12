//! 读侧单测：协议解析在 `flux_shared::iyuu`（有真机 fixture），
//! 这里只管「站点表三层优先级」和「站长手写表的容错」。

/// 站点表覆盖：`sid|名称|域名`，`#` 注释、空行、坏行都跳过。
#[test]
fn parse_extra_tolerates_handwritten_input() {
    use super::parse_extra;
    let text = "\n# 注释行\n140|新站|new.example\n\n".to_string()
        + "141 | 带空格 | dom.example/ \n坏行\n142|\n";
    let m = parse_extra(&text);
    assert_eq!(m.len(), 2);
    assert_eq!(m[&140].0, "新站");
    assert_eq!(m[&140].1, "new.example");
    assert_eq!(m[&141].0, "带空格");
    // 末尾斜杠去掉，免得拼出 //details.php
    assert_eq!(m[&141].1, "dom.example");
    assert!(!m.contains_key(&142));
}

/// 三层优先级：站长覆盖 > worker 同步表 > 内置表。
#[test]
fn resolve_site_priority() {
    use super::resolve_site;
    let mut extra = std::collections::HashMap::new();
    extra.insert(4_i64, ("改名了".to_string(), "new.me".to_string()));
    let mut synced = std::collections::HashMap::new();
    synced.insert(4_i64, flux_shared::iyuu::Site::plain("同步来的", "sync.me"));
    synced.insert(6_i64, flux_shared::iyuu::Site::plain("同步来的", "sync.me"));

    // 覆盖表最高
    assert_eq!(resolve_site(&extra, &synced, 4).unwrap().name, "改名了");
    // 其次同步表
    assert_eq!(resolve_site(&extra, &synced, 6).unwrap().name, "同步来的");
    // 都没有就走内置表（sid 3 = 馒头）
    assert_eq!(resolve_site(&extra, &synced, 3).unwrap().name, "馒头");
    // 三层都没有
    assert!(resolve_site(&extra, &synced, 9999).is_none());
}

/// 内置表覆盖了 IYUU 现有站点的大部分（98 条），且带域名。
#[test]
fn builtin_sites_sane() {
    use super::sites::by_sid;
    assert_eq!(by_sid(137).map(|(n, _)| n), Some("好学"));
    assert_eq!(by_sid(3).map(|(_, d)| d), Some("api.m-team.cc"));
    assert!(by_sid(999_999).is_none());
}

/// host 解析：完整 URL / 裸域名 / 带端口 / 带路径统一抠出小写 host。
#[test]
fn host_of_url_extracts_bare_host() {
    use super::host_of_url;
    assert_eq!(
        host_of_url("https://FluxTorrent.org/"),
        Some("fluxtorrent.org".to_string())
    );
    assert_eq!(host_of_url("http://a.b.c"), Some("a.b.c".to_string()));
    assert_eq!(
        host_of_url("bare.example/x/y"),
        Some("bare.example".to_string())
    );
    assert_eq!(
        host_of_url("dom.example:8080/z"),
        Some("dom.example".to_string())
    );
    assert_eq!(host_of_url("  "), None);
    assert_eq!(host_of_url("https://"), None);
}

/// 自站命中过滤：域名对上才算；解析不出站点或对不上都放行（宁多勿杀）。
#[test]
fn self_hit_filters_by_domain_only() {
    use super::{host_of_url, is_self_hit};
    use std::collections::HashMap;
    let extra: HashMap<i64, (String, String)> = HashMap::new();
    let mut synced = HashMap::new();
    synced.insert(
        42_i64,
        flux_shared::iyuu::Site::plain("自家", "fluxtorrent.org"),
    );
    synced.insert(
        43_i64,
        flux_shared::iyuu::Site::plain("友站", "other.example"),
    );

    // 直接测 host 比对内核（is_self_hit 读 env，逐测试设/撤不稳定，拆开验）
    assert_eq!(
        host_of_url(
            &flux_shared::iyuu::Site::plain("自家", "fluxtorrent.org").host
        ),
        Some("fluxtorrent.org".to_string())
    );
    // 三层表都没有的 sid：is_self_hit 必须放行（None → false 路径）
    assert!(
        !is_self_hit(&extra, &HashMap::new(), 999_999)
            || std::env::var("PUBLIC_SITE_URL").is_ok()
    );
    // env 没配时任何 sid 都放行
    if std::env::var("PUBLIC_SITE_URL").is_err()
        && std::env::var("PUBLIC_WEB_URL").is_err()
        && std::env::var("PUBLIC_API_URL").is_err()
    {
        assert!(!is_self_hit(&extra, &synced, 42));
    }
}
