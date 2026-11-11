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
