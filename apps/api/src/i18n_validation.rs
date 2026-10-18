//! 参数化校验模板的渲染（D9 机制，2026-10-14 立项落地）。
//!
//! 背景：校验详情本地化靠 `validation_details.tsv` 按「整句原句」查表——
//! 静态字面量零改动直查；但 `format!` 拼出来的带参句子没有固定原句可查，
//! 只能死写中文。`validation_i18n_guard` 的棘轮（只许降不许增）锁的就是
//! 这类，而 211 处存量说明这不是单批能清的——需要本机制：key 查模板、
//! args 在响应期插值，三语模板进表，调用点不再碰 format!。
//!
//! 独立成文件的原因：`i18n.rs` 已在行数棘轮存量上（只许瘦不许胖），
//! 新机制进新文件（同 ui_design_guard_motion.mjs 的先例）。

use crate::i18n::Locale;

/// 模板表：key → (zh-CN, zh-TW, en)；占位符 `{name}`。
const TEMPLATES_TSV: &str = include_str!("../i18n/validation_templates.tsv");

fn template_table() -> &'static std::collections::HashMap<
    &'static str,
    (&'static str, &'static str, &'static str),
> {
    static TABLE: std::sync::OnceLock<
        std::collections::HashMap<&'static str, (&'static str, &'static str, &'static str)>,
    > = std::sync::OnceLock::new();
    TABLE.get_or_init(|| {
        let mut m = std::collections::HashMap::new();
        for line in TEMPLATES_TSV.lines() {
            let t = line.trim_end();
            if t.is_empty() || t.starts_with('#') {
                continue;
            }
            let mut cols = t.split('\t');
            if let (Some(key), Some(zh), Some(tw), Some(en)) =
                (cols.next(), cols.next(), cols.next(), cols.next())
            {
                m.insert(key.trim(), (zh.trim(), tw.trim(), en.trim()));
            }
        }
        m
    })
}

/// 模板条目数（测试与守卫对账用）。
pub fn template_count() -> usize {
    template_table().len()
}

/// 按 locale 渲染参数化校验详情。
///
/// **表里没有该 key 就原样回退 key 本身**：宁可露 key，绝不 panic、绝不
/// 现场编文（与 localized_detail 的「宁露中文不编文」同一哲学）。
/// 占位符 `{name}` 按 args 顺序替换；未提供的占位符原样保留（显性可见，
/// 便于发现模板与 args 不匹配）。
pub fn render_validation_key(
    key: &str,
    locale: Locale,
    args: &[(&str, String)],
) -> String {
    let tpl = match locale {
        Locale::ZhCn => template_table().get(key).map(|t| t.0),
        Locale::ZhTw => template_table().get(key).map(|t| t.1),
        Locale::En => template_table().get(key).map(|t| t.2),
    }
    .unwrap_or(key);
    let mut out = tpl.to_string();
    for (name, value) in args {
        out = out.replace(&format!("{{{name}}}"), value);
    }
    out
}

#[cfg(test)]
mod tests {
    use super::{render_validation_key, template_count};

    /// 表非空：机制落地至少有一条模板（空表 = 忘了 TSV 或解析坏了）
    #[test]
    fn template_table_is_seeded() {
        assert!(template_count() > 0, "参数化模板表不应为空");
    }

    /// zh-CN 直出 + 占位符插值
    #[test]
    fn renders_zh_cn_with_args() {
        let out = render_validation_key(
            "torrent.buy.insufficient",
            Locale::ZhCn,
            &[("price", "42".to_string())],
        );
        assert!(out.contains("42"), "占位符应被替换：{out}");
        assert!(out.contains("魔力不足"), "zh-CN 模板应直出：{out}");
        assert!(!out.contains("{price}"), "不应残留占位符：{out}");
    }

    /// en 模板走翻译列；zh-TW 走繁体列
    #[test]
    fn renders_by_locale() {
        let en = render_validation_key(
            "torrent.buy.insufficient",
            Locale::En,
            &[("price", "42".to_string())],
        );
        assert!(en.contains("paid"), "en 模板应生效：{en}");
        let tw = render_validation_key(
            "torrent.buy.insufficient",
            Locale::ZhTw,
            &[("price", "42".to_string())],
        );
        assert!(tw.contains("付費"), "zh-TW 模板应生效：{tw}");
    }

    /// 未知 key 回退 key 本身（不 panic、不编文）
    #[test]
    fn unknown_key_falls_back_to_key() {
        let out = render_validation_key("no.such.key", Locale::ZhCn, &[]);
        assert_eq!(out, "no.such.key");
    }
}
