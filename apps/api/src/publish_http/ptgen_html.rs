//! PT-Gen 返回 HTML → 纯文本（从 ptgen.rs 按 300 行门禁拆出，纯搬运）。

/// 简易 HTML → 纯文本（PT-Gen 返回物）：块级标签转行、剥其余标签、解常见实体
pub(super) fn html_to_text(html: &str) -> String {
    let mut s = html
        .replace("<br>", "\n")
        .replace("<br/>", "\n")
        .replace("<br />", "\n")
        .replace("</p>", "\n")
        .replace("</div>", "\n")
        .replace("</tr>", "\n")
        .replace("</li>", "\n")
        .replace("<li>", "- ")
        .replace("</td>", "  ")
        .replace("</th>", "  ");
    // 剥离其余标签（PT-Gen 输出为受信源生成的受控 HTML，逐字符状态机即可）
    let mut out = String::with_capacity(s.len());
    let mut in_tag = false;
    for ch in s.chars() {
        match ch {
            '<' => in_tag = true,
            '>' => in_tag = false,
            c if !in_tag => out.push(c),
            _ => {}
        }
    }
    s = out;
    for (ent, ch) in [
        ("&nbsp;", " "),
        ("&amp;", "&"),
        ("&lt;", "<"),
        ("&gt;", ">"),
        ("&quot;", "\""),
        ("&#39;", "'"),
    ] {
        s = s.replace(ent, ch);
    }
    // 折叠空行 + 去行尾空白
    let mut lines: Vec<String> = Vec::new();
    for line in s.lines() {
        let t = line.trim_end();
        if t.is_empty() && lines.last().map(String::is_empty).unwrap_or(true) {
            continue;
        }
        lines.push(t.to_string());
    }
    lines.join("\n").trim().to_string()
}
