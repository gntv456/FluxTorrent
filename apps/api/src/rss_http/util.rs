//! RSS 出口共用纯函数（300 行门禁）：XML 转义 / id 串解析 / 体积格式化。
//! 主订阅源与论坛订阅源共用；解析口径「给了非法值就报错」不静默全量。

/// XML 文本节点转义（三个必转字符；属性场景本出口不涉及）
pub fn xml_escape(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
}

/// 逗号分隔 id 串 → 数组。空/未给 = 不过滤（Ok(None)）；
/// 给了值却有解析不出的 token → Err（此前静默丢弃，全非法时等于不加谓词、
/// 返回全站订阅源，比报错危险）
pub fn parse_ids(s: Option<&str>) -> Result<Option<Vec<i32>>, String> {
    let s = match s {
        Some(v) => v.trim(),
        None => return Ok(None),
    };
    if s.is_empty() {
        return Ok(None);
    }
    let mut ids: Vec<i32> = Vec::new();
    for p in s.split(',') {
        let p = p.trim();
        if p.is_empty() {
            continue;
        }
        match p.parse::<i32>() {
            Ok(v) => {
                if !ids.contains(&v) {
                    ids.push(v);
                }
            }
            Err(_) => return Err(format!("参数非法：{p}")),
        }
    }
    Ok(if ids.is_empty() { None } else { Some(ids) })
}

pub fn format_size(b: i64) -> String {
    let gb = b as f64 / 1024f64.powi(3);
    if gb >= 1.0 {
        format!("{gb:.2} GB")
    } else {
        format!("{:.0} MB", b as f64 / 1024f64.powi(2))
    }
}

#[cfg(test)]
mod tests {
    use super::parse_ids;

    #[test]
    fn parse_ids_multi_and_invalid() {
        assert_eq!(parse_ids(Some("1,2, 3")), Ok(Some(vec![1, 2, 3])));
        assert_eq!(parse_ids(Some("5")), Ok(Some(vec![5])));
        assert_eq!(parse_ids(Some("")), Ok(None));
        assert_eq!(parse_ids(None), Ok(None));
        // 给了值却解析不出来：报错，不再退化成「不过滤 = 返回全站」
        assert!(parse_ids(Some("x,y")).is_err());
        assert!(parse_ids(Some("1,abc")).is_err());
        // 重复值合并，避免生成重复谓词
        assert_eq!(parse_ids(Some("2,2")), Ok(Some(vec![2])));
        // 旧字段兼容由 handler 单独处理
    }
}
