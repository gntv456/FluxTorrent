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
            let allowed: Vec<String> = m
                .options
                .as_ref()
                .and_then(|o| o.as_array())
                .map(|a| {
                    a.iter()
                        .filter_map(|x| {
                            x.get("v")
                                .and_then(|v| v.as_str())
                                .map(String::from)
                        })
                        .collect()
                })
                .unwrap_or_default();
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

// ============================================================================
