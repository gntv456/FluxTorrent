//! engine.rs 的单元测试（300 行门禁：从 settings_http/engine.rs 拆出）。
//!
//! 校验引擎的每条 rule 分支都在这里钉一套「接受集」——保存口放开的形状，
//! 必须与真正读它的界面一致，否则症状是「存了、回显了、界面没变化」。

use super::engine::validate_field;
use super::meta::MetaRow;

fn text_meta(options: Option<serde_json::Value>) -> MetaRow {
    MetaRow {
        name: "upload_title_pattern".into(),
        kind: "text".into(),
        label_zh: None,
        label_en: None,
        hint: None,
        unit: None,
        min: None,
        max: None,
        step: None,
        options,
        secret: false,
        readonly: false,
        group_key: Some("torrent".into()),
        min_class: None,
        grp: "torrent".into(),
        value: String::new(),
        updated_at: chrono::Utc::now(),
    }
}

/// rule=regex：坏正则必须在**保存**这一刻被拒，而不是等会员发种才发现。
///
/// 反面教材也是实测出来的：本用例第一版拿 `^[(\d]` 当「坏正则」，Rust
/// regex 收下了它（`[` 开字符类、`\d` 是类里的合法成员）⇒ 只钉真正编译
/// 不过的形状。
#[test]
fn regex_rule_rejects_bad_pattern() {
    let m = text_meta(Some(serde_json::json!({ "rule": "regex" })));
    let e = validate_field(&m, "(unclosed").unwrap_err();
    assert!(e.contains("须为合法正则表达式"), "实得={e}");
    assert!(validate_field(&m, "a{2,1}").is_err(), "区间倒挂须拒");
    // 后查与反向引用是「Java 写法」里最常见的两条本站引擎不支持项
    assert!(validate_field(&m, r"(?<=x)y").is_err(), "后查须拒");
    assert!(validate_field(&m, r"^(a)\1$").is_err(), "反向引用须拒");
    // 合法的放行（就是 0426 出厂说明里给的那条示例格式）
    assert!(
        validate_field(&m, r"^[^.]+\.[0-9]{4}\.(1080p|2160p)").is_ok(),
        "示例格式须收下"
    );
    // 空串=不校验，既有语义不动
    assert!(validate_field(&m, "").is_ok(), "空值仍表示不校验");
}

/// 正控：不带 rule 的 text 键不受影响——否则这条会把全站文本设定拦死。
#[test]
fn text_without_rule_still_accepts_anything() {
    let m = text_meta(None);
    assert!(validate_field(&m, "(unclosed").is_ok());
}

/// rule=url_or_path（0456 背景图）的接受集必须与前端 `lib/site-background.ts`
/// 的拒绝集严丝合缝。那份文件的读者是浏览器：只认站内 `/…` 与 http(s)://，
/// 别的一律不渲染背景。保存口每放开一条它丢弃的形状，站长侧的症状都是
/// 「存了、界面回显了、背景却没出现」——没有报错可查的假开关。
/// 语料直接抄 site-background.test.ts 里那条 bad 清单，两边同尺才算数。
#[test]
fn url_or_path_accepts_exactly_what_the_renderer_paints() {
    let m = text_meta(Some(serde_json::json!({ "rule": "url_or_path" })));
    for ok in [
        "/backgrounds/site-tide-photo.jpg",
        "https://cdn.example.com/bg.jpg",
        "http://127.0.0.1:3000/bg.png?a=1",
        // 前端 /^https?:\/\//i 大小写不敏感，这里也得收
        "HTTPS://CDN.Example.com/bg.jpg",
    ] {
        assert!(validate_field(&m, ok).is_ok(), "须放行 {ok}");
    }
    let long = format!("https://x/{}", "a".repeat(2100));
    for bad in [
        "//cdn.example/bg.jpg",
        "/\\bg.jpg",
        "/a b.jpg",
        "cdn.example/bg.jpg",
        "127.0.0.1:3000/bg.jpg",
        "javascript:alert(1)",
        "data:image/png;base64,AAAA",
        "https://x/a b.jpg",
        "https://x/\"a.jpg",
        long.as_str(),
    ] {
        let e = validate_field(&m, bad).unwrap_err();
        assert!(!e.is_empty(), "须拦下 {bad}");
    }
    // 空值=不启用，与 rule=url 的既有语义一致
    assert!(validate_field(&m, "").is_ok(), "空值仍表示不启用");
}

/// 正控：rule=url 的键（webhook 等后端消费的地址）照旧容忍裸 host:port，
/// 上面那条收紧只管 url_or_path，别把别的键拦死。
#[test]
fn url_rule_keeps_host_port_leniency() {
    let m = text_meta(Some(serde_json::json!({ "rule": "url" })));
    assert!(validate_field(&m, "127.0.0.1:3000/hooks/a").is_ok());
    assert!(
        validate_field(&m, "/backgrounds/a.jpg").is_err(),
        "站内相对路径仍只有 url_or_path 收"
    );
}

fn enum_meta(options: serde_json::Value) -> MetaRow {
    let mut m = text_meta(None);
    m.kind = "enum".into();
    m.options = Some(options);
    m
}

/// 枚举选项两种历史形态都要校验。对象形（captcha_provider 用的
/// `{options:[{label,value}]}`）曾因只认数组形而放行任意值——实测
/// 后台可存 `bogus`，注册页不渲染验证码、提交必报「验证码错误」。
#[test]
fn enum_accepts_both_option_shapes_and_rejects_unknown() {
    let arr = serde_json::json!([
        {"l": "开", "v": "open"},
        {"l": "关", "v": "off"}
    ]);
    let m_arr = enum_meta(arr);
    assert!(validate_field(&m_arr, "open").is_ok());
    assert!(validate_field(&m_arr, "bogus").is_err());
    assert!(validate_field(&m_arr, "").is_err(), "空值不是合法枚举");

    let obj = serde_json::json!({"options": [
        {"label": "算术", "value": "none"},
        {"label": "Turnstile", "value": "turnstile"}
    ]});
    let m_obj = enum_meta(obj);
    assert!(validate_field(&m_obj, "none").is_ok());
    assert!(validate_field(&m_obj, "turnstile").is_ok());
    let e = validate_field(&m_obj, "bogus").unwrap_err();
    assert!(e.contains("none"), "错误须列出允许值，实得={e}");
}
