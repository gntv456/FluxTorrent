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
                    // 背景图一类的「同源优先」地址（0456）：站内相对路径
                    // （/backgrounds/x.jpg）或 http(s) 绝对地址。相对路径让
                    // 背景随访问入口同源加载，不必把公网域名写进配置。
                    //
                    // 这里故意不复用 looks_like_url：那条容忍裸 host:port 是给
                    // **后端消费**的地址用的，而背景图的读者是浏览器——前端
                    // lib/site-background.ts 只认 `/…` 与 http(s)://，其余一律
                    // 不渲染。保存口放开浏览器用不了的值，症状就是「存了、回显
                    // 了、界面没变化」的假开关。接受集与那条文件的拒绝集同尺。
                    "url_or_path" => {
                        let low = v.to_ascii_lowercase();
                        let rel = low.starts_with('/')
                            && !v.starts_with("//")
                            && !v.starts_with("/\\");
                        let abs = low.starts_with("http://")
                            || low.starts_with("https://");
                        if !rel && !abs {
                            return Err(
                                "须为站内路径（/…）或 http(s) 绝对地址".into(),
                            );
                        }
                        let bad_char = v
                            .chars()
                            .any(|c| c.is_whitespace() || "\"'\\".contains(c));
                        if bad_char {
                            return Err("地址不能含空白、引号或反斜杠".into());
                        }
                        if v.len() > 2048 {
                            return Err("地址长度不得超过 2048".into());
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
