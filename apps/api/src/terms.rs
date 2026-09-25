//! 术语覆盖层（四审 L7：站长改不动「种子」这类固有词）。
//!
//! 规则存在 `site_terms` 表里（0205），这里只做一件事：把启用中的规则缓成
//! 进程内快照，并在**文案出口**上一次正向替换。出口共两处：
//! - HTTP 错误信封（`errors.rs::error_response`）——390 条中文校验串从这儿出去；
//! - 前端文案（`/site-profile` 下发规则，`apps/web/i18n/apply-terms.ts` 过字典）。
//!
//! 为什么是进程级静态而不是塞进 AppState：`ResponseError::error_response(&self)`
//! 的签名拿不到 state（trait 只给 &self），与 `i18n.rs` 用 task-local 传 locale
//! 是同一类取舍。
//!
//! 写侧（`staff_http/terms.rs`）落库后**立刻** `reload()`，不走 TTL——
//! 「改完看不到变化」这个坑在装机改密的 5s 状态缓存上已经踩过一次。

use std::sync::RwLock;

/// `(canonical, replacement)` 快照；顺序不变量（长词优先）由 `install()` 保证。
static RULES: RwLock<Vec<(String, String)>> = RwLock::new(Vec::new());

/// 规则条数上限：单次替换对每条规则做一次前缀判断，文案出口在错误路径上，
/// 不给它无上限的乘积。200 条足够一个站点把自己的词汇表改写完。
pub(crate) const MAX_RULES: i64 = 200;

/// 装载快照并**在此处**排成「长词优先」：这个顺序是替换正确性的一部分
/// （否则「种子」会抢在「种子文件」之前咬掉一半），所以由本模块保证，
/// 不依赖调用方/SQL 的 ORDER BY。
pub(crate) fn install(mut rules: Vec<(String, String)>) {
    rules.sort_by(|a, b| b.0.len().cmp(&a.0.len()).then_with(|| a.0.cmp(&b.0)));
    if let Ok(mut g) = RULES.write() {
        *g = rules;
    }
}

/// 从库里重载快照，返回生效条数。读库失败时保持旧快照（文案宁可旧，不可空）。
pub(crate) async fn reload(db: &sqlx::PgPool) -> usize {
    let rows = sqlx::query_as::<_, (String, String)>(
        "SELECT canonical, replacement FROM site_terms \
         WHERE enabled \
         ORDER BY length(canonical) DESC, canonical \
         LIMIT $1",
    )
    .bind(MAX_RULES)
    .fetch_all(db)
    .await
    .unwrap_or_default();
    let n = rows.len();
    install(rows);
    n
}

/// 当前快照里的规则数（诊断与测试用）。
pub(crate) fn rule_count() -> usize {
    RULES.read().map(|g| g.len()).unwrap_or(0)
}

/// 找出文本里所有 `{...}` 插值占位符的字节区间（`{` 到其后第一个 `}`）。
///
/// 为什么必须保护：改写发生在**模板**上（字典叶子、后端已格式化的提示语），
/// 而字典里 `{magic}` 是站点货币名出口、`{n}` 是插值变量。若某条规则的原词
/// 正好是 `magic` 或 `n`，不保护就会把 `{magic}` 撕成 `{积分}` —— 前端 `fmt()`
/// 取不到变量，文案当场少一块。占位符整段视为原子，不做任何改写。
fn placeholder_spans(text: &str) -> Vec<(usize, usize)> {
    let mut spans = Vec::new();
    let mut open: Option<usize> = None;
    for (i, c) in text.char_indices() {
        match (c, open) {
            ('{', _) => open = Some(i),
            ('}', Some(s)) => {
                spans.push((s, i + c.len_utf8()));
                open = None;
            }
            _ => {}
        }
    }
    spans
}

/// 位置 `i` 是否落在某个占位符**内部**（区间端点本身仍可匹配，
/// 因为规则已禁止含花括号，落在 `{`/`}` 上的匹配不会撕开占位符）。
fn in_span(spans: &[(usize, usize)], i: usize) -> bool {
    spans.iter().any(|(s, e)| i > *s && i < *e)
}

/// 按当前规则改写一段文案。零规则走快速路径：原样返回，不重新分配。
///
/// 单次正向扫描：命中后游标跳过**源词**长度，替换文本不再参与后续匹配，
/// 因此「种子→资源」「资源→素材」这种互指规则也只走一遍、不会级联或死循环。
pub(crate) fn apply(text: &str) -> String {
    let Ok(rules) = RULES.read() else {
        return text.to_string();
    };
    if rules.is_empty() || text.is_empty() {
        return text.to_string();
    }
    let spans = placeholder_spans(text);
    let mut out = String::with_capacity(text.len());
    // i 始终落在字符边界上：起点是 0，命中跳过整条源词，未命中跳一个 char
    let mut i = 0;
    while i < text.len() {
        let hit = if in_span(&spans, i) {
            None
        } else {
            rules.iter().find(|(f, _)| text[i..].starts_with(f))
        };
        match hit {
            Some((from, to)) => {
                out.push_str(to);
                i += from.len();
            }
            None => {
                let c = text[i..].chars().next().unwrap_or('\u{fffd}');
                out.push(c);
                i += c.len_utf8();
            }
        }
    }
    out
}

/// 词条本身（canonical / replacement 都过这条）。canonical 就是文案里那个词，
/// 所以它是主键、也是包预置段里的引用名——不做 ASCII 键名那一层，
/// 多一层映射就会有多份真值。
///
/// 拒含 `{`/`}`：字典里 `{n}`、`{magic}` 是插值占位符，规则一旦能把「{magic}」
/// 里的片段换掉，前端 `fmt()` 就拿不到变量了。
pub(crate) fn valid_term_word(w: &str) -> bool {
    let t = w.trim();
    !t.is_empty()
        && t.chars().count() <= 20
        && !t.contains('{')
        && !t.contains('}')
}

#[cfg(test)]
mod tests {
    use super::*;

    /// RULES 是进程级静态，cargo 的测试线程会并发跑 ⇒ 必须串行，
    /// 否则 A 测试刚装载的规则会被 B 测试清空（假红/假绿双向）。
    static TEST_GUARD: std::sync::Mutex<()> = std::sync::Mutex::new(());
    fn guard() -> std::sync::MutexGuard<'static, ()> {
        TEST_GUARD.lock().unwrap_or_else(|e| e.into_inner())
    }

    fn set(rules: &[(&str, &str)]) {
        install(
            rules
                .iter()
                .map(|(a, b)| (a.to_string(), b.to_string()))
                .collect(),
        );
    }

    #[test]
    fn no_rules_is_identity() {
        let _g = guard();
        set(&[]);
        assert_eq!(apply("今天已经签到过啦"), "今天已经签到过啦");
    }

    #[test]
    fn rewrites_every_occurrence_once() {
        let _g = guard();
        // 「种子」出现两次都要改；这是 L7 的正面诉求（统一改叫法）
        set(&[("种子", "资源")]);
        assert_eq!(apply("该种子不是悬赏帖"), "该资源不是悬赏帖");
        assert_eq!(apply("种子种子"), "资源资源");
    }

    #[test]
    fn longest_rule_wins() {
        let _g = guard();
        // 短词不能把长词咬坏；而且**给的顺序是反的**也要成立——
        // 长词优先是 install() 的不变量，不是调用方的义务。
        set(&[("种子", "资源"), ("种子文件", "资源包")]);
        assert_eq!(apply("请上传种子文件"), "请上传资源包");
        set(&[("种子文件", "资源包"), ("种子", "资源")]);
        assert_eq!(apply("请上传种子文件"), "请上传资源包");
    }

    #[test]
    fn mutual_rules_do_not_cascade() {
        let _g = guard();
        // 互指规则：替换出的文本不再被扫描，因此不会级联成第三种词
        set(&[("资源", "种子"), ("种子", "资源")]);
        assert_eq!(apply("种子"), "资源");
        assert_eq!(apply("资源的种子"), "种子的资源");
    }

    #[test]
    fn mixed_ascii_and_multibyte_survive() {
        let _g = guard();
        set(&[("保种", "留存")]);
        // 中英混排 + emoji：字符边界算错就是 panic 或乱码
        assert_eq!(apply("keep 保种🌱 协作"), "keep 留存🌱 协作");
    }

    #[test]
    fn placeholder_tokens_are_atomic() {
        let _g = guard();
        // 规则原词撞上占位符变量名：`{magic}`（站点货币名出口）不能被撕开，
        // 否则前端 fmt() 取不到变量、文案当场少一块
        set(&[("magic", "积分"), ("n", "数量")]);
        assert_eq!(apply("获得 {magic} 与 {n}"), "获得 {magic} 与 {n}");
        // 但占位符**外面**的同名词照常被改
        assert_eq!(apply("magic 与 {magic}"), "积分 与 {magic}");
    }

    #[test]
    fn placeholder_braces_are_rejected() {
        let _g = guard();
        assert!(valid_term_word("魔力"));
        assert!(!valid_term_word("魔{力"));
        assert!(!valid_term_word("magic}"));
        assert!(!valid_term_word("   "));
        assert!(!valid_term_word(&"超".repeat(21)));
    }
}
