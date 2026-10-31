//! 校验引擎（§4.2：type / range / enum / url / required）。
//! 从 settings_http.rs 按域拆出。

use super::meta::*;

// 校验引擎（§4.2：type / range / enum / url / required）
// ============================================================================

pub(super) fn is_hex_color(s: &str) -> bool {
    let b = s.as_bytes();
    b.len() == 7 && b[0] == b'#' && b[1..].iter().all(|c| c.is_ascii_hexdigit())
}

/// URL / 主机地址校验：容忍无协议的 host:port（如 127.0.0.1:3000），拒绝空白与无点主机
fn looks_like_url(s: &str) -> bool {
    let t = s.trim();
    if t.is_empty() || t.contains(char::is_whitespace) {
        return false;
    }
    let rest = t
        .strip_prefix("https://")
        .or_else(|| t.strip_prefix("http://"))
        .unwrap_or(t);
    let host = rest.split('/').next().unwrap_or("");
    let host_no_port = host.rsplit_once(':').map(|(h, _)| h).unwrap_or(host);
    !host_no_port.is_empty()
        && (host_no_port.contains('.') || host_no_port == "localhost")
}

fn fmt_num(n: f64) -> String {
    if n.fract() == 0.0 {
        format!("{}", n as i64)
    } else {
        format!("{n}")
    }
}

/// 校验并归一化单个字段值；`Ok(值)` / `Err(错误说明)`
pub(super) fn validate_field(m: &MetaRow, raw: &str) -> Result<String, String> {
    let v = raw.trim().to_string();
    let rule = m
        .options
        .as_ref()
        .and_then(|o| o.get("rule"))
        .and_then(|r| r.as_str())
        .unwrap_or("");
    let required = m
        .options
        .as_ref()
        .and_then(|o| o.get("required"))
        .and_then(|r| r.as_bool())
        .unwrap_or(false);

    match m.kind.as_str() {
        "number" => {
            if v.is_empty() {
                return Err("不能为空".into());
            }
            let n: f64 = v.parse().map_err(|_| "必须是数字".to_string())?;
            if let Some(min) = m.min {
                if n < min {
                    return Err(format!("不得小于 {}", fmt_num(min)));
                }
            }
            if let Some(max) = m.max {
                if n > max {
                    return Err(format!("不得大于 {}", fmt_num(max)));
                }
            }
            if m.step.unwrap_or(1.0) == 1.0 && n.fract() != 0.0 {
                return Err("必须是整数".into());
            }
        }
        "yesno" => {
            if v != "yes" && v != "no" {
                return Err("只能取 yes / no".into());
            }
        }
        "enum" => {
            // settings_meta.options 有两种历史形态：
            //   [{l,v}]                    —— registration_mode 等
            //   {options:[{label,value}]} —— captcha_provider 等
            // 旧实现只认数组形，对象形 as_array()=None ⇒ allowed 为空 ⇒
            // **任意字符串都能存**（实测 captcha_provider=bogus 可保存，
            // 随后注册页不渲染验证码、提交必失败）。两种形态都要校验。
            let mut allowed: Vec<String> = Vec::new();
            if let Some(o) = m.options.as_ref() {
                let arr = o
                    .as_array()
                    .or_else(|| o.get("options").and_then(|x| x.as_array()));
                if let Some(a) = arr {
                    for x in a {
                        let val = x
                            .get("v")
                            .or_else(|| x.get("value"))
                            .and_then(|v| v.as_str());
                        if let Some(v) = val {
                            allowed.push(v.to_string());
                        }
                    }
                }
            }
            if !allowed.is_empty() && !allowed.iter().any(|x| x == &v) {
                return Err(format!("取值须为 {}", allowed.join(" / ")));
            }
        }
        "color" => {
            if !is_hex_color(&v) {
                return Err("须为合法 hex 颜色，如 #FFD700".into());
            }
        }
        "classlevel" => {
            if v.is_empty() {
                return Err("不能为空".into());
            }
            let n: i32 = v.parse().map_err(|_| "必须是等级数字".to_string())?;
            if !(0..=100).contains(&n) {
                return Err("等级取值 0-100".into());
            }
        }
        "password" => {
            if raw.len() > 4096 {
                return Err("长度不得超过 4096".into());
            }
        }
        "textarea" => {
            if raw.len() > 65535 {
                return Err("长度不得超过 65535".into());
            }
        }
        // text / pair
        _ => {
            if required && v.is_empty() {
                return Err("不能为空".into());
            }
            if v.len() > 4096 {
                return Err("长度不得超过 4096".into());
            }
            if !v.is_empty() {
                match rule {
                    "url" => {
                        if !looks_like_url(&v) {
                            return Err("须为合法 URL 或 host:port".into());
                        }
                    }
                    "csv_ids" => {
                        if !v
                            .split(',')
                            .all(|p| p.trim().parse::<i64>().is_ok())
                        {
                            return Err("须为逗号分隔的数字 ID".into());
                        }
                    }
                    // 坏正则的代价不对称：存进去那一刻毫无信号，之后**每一次**
                    // 会员发种都被「站点命名正则非法，请联系站长」挡住
                    // （publish_http/upload_precheck.rs 的 check_quality 在真实
                    // 上传路径上），而站长界面一切正常。所以在保存这一刻就编译
                    // 一次。引擎就是 `regex` crate——面板教的语法必须与它一致，
                    // 报错里点名两条最常踩的不支持项（后查 / 反向引用）。
                    "regex" => {
                        if let Err(e) = regex::Regex::new(&v) {
                            return Err(format!(
                                "须为合法正则表达式（本站用 Rust regex 语法，\
                                 不支持后查 (?<=) 与反向引用 \\1）：{e}"
                            ));
                        }
                    }
                    _ => {}
                }
            }
        }
    }
    // 密文保留原始字节（避免误 trim 密码首尾空格），其余统一 trim
    Ok(if m.kind == "password" {
        raw.to_string()
    } else {
        v
    })
}

#[cfg(test)]
mod tests {
    use super::*;

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
}

// ============================================================================
