//! 发布简介首图提取（0159：未填封面 URL 时回落简介第一张图）。
//! 从 ptgen.rs 按域拆出（300 行门禁）。

/// 简介里的第一张图 URL：`<img src="...">` / `![](url)` / `[img]url[/img]`
/// 三种写法都认（descr 是富文本 HTML + BBCode 混合的存量现实）。
/// 仅接受 http(s) 外链，防 `javascript:`/相对路径注入。
pub(super) fn first_descr_image(descr: Option<&str>) -> Option<String> {
    let d = descr?.trim();
    if d.is_empty() {
        return None;
    }
    let take_url = |s: &str| -> Option<String> {
        let u = s.trim().trim_matches(|c| c == '"' || c == '\'').trim();
        let ok = u.starts_with("http://") || u.starts_with("https://");
        ok.then(|| u.chars().take(2000).collect())
    };
    // 三种写法按「文档顺序」取最早出现（0159：用户在简介里看到的第一张图）。
    // src= 后紧跟引号：split 首段是空串，find 跳过空段取 URL。
    let mut best: Option<(usize, String)> = None;
    if let Some(pos) = d.find("src=") {
        let seg = d[pos + 4..]
            .split(['"', '\'', '>', ' '])
            .find(|x| !x.is_empty());
        if let Some(u) = seg.and_then(|x| take_url(x)) {
            best = Some((pos, u));
        }
    }
    if let Some(pos) = d.find("](") {
        if let Some(u) =
            d[pos + 2..].split(')').next().and_then(|x| take_url(x))
        {
            if best.as_ref().map_or(true, |(p, _)| pos < *p) {
                best = Some((pos, u));
            }
        }
    }
    if let Some(pos) = d.find("[img]") {
        if let Some(u) = d[pos + 5..]
            .split("[/img]")
            .next()
            .and_then(|x| take_url(x))
        {
            if best.as_ref().map_or(true, |(p, _)| pos < *p) {
                best = Some((pos, u));
            }
        }
    }
    best.map(|(_, u)| u)
}

#[cfg(test)]
mod descr_image_tests {
    use super::first_descr_image;

    #[test]
    fn picks_earliest_in_document_order() {
        let d = "前言 [img]https://a/1.png[/img] <img src='https://b/2.jpg'>";
        assert_eq!(
            first_descr_image(Some(d)).as_deref(),
            Some("https://a/1.png")
        );
    }

    #[test]
    fn html_src() {
        assert_eq!(
            first_descr_image(Some(r#"<img src="https://x/p.jpg" alt>"#))
                .as_deref(),
            Some("https://x/p.jpg")
        );
    }

    #[test]
    fn bbcode_img() {
        assert_eq!(
            first_descr_image(Some("看这个 [img]https://c/i.png[/img] 好"))
                .as_deref(),
            Some("https://c/i.png")
        );
    }

    #[test]
    fn rejects_non_http() {
        assert_eq!(
            first_descr_image(Some("[img]javascript:alert(1)[/img]")),
            None
        );
        assert_eq!(first_descr_image(Some("src=\"relative/x.png\"")), None);
    }

    #[test]
    fn none_when_empty() {
        assert_eq!(first_descr_image(None), None);
        assert_eq!(first_descr_image(Some("纯文字无图")), None);
    }
}
