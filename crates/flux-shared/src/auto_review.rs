//! 自动审种·规则内核（《自动审种功能策划案》v1.3 §2/§9）。
//!
//! 【为什么在 flux-shared】裁决出口（`services::review_decide`）在 api，
//! 取件与深检在 worker——规则评估是两边共用的唯一实现，放任何一侧都会
//! 漂移（本 crate 头注释立 crate 时的同款理由）。评估器刻意写成
//! **纯函数**：输入 `TorrentSnapshot` + `PublisherProfile` + `Config`，
//! 输出命中与证据，不触网不查库——这是回放（replay）与金标准回归集
//! （策划案 §18.3）能成立的前提。
//!
//! 裁决同尺纪律（策划案 §1.2 第 1 条）：本模块只**评估**，不裁决。
//! 落库走 api 侧 `services::review_decide`（或影子模式只写台账），
//! 引擎永不直接 UPDATE torrents。

use serde_json::json;

/// 一条规则的评估结果。`detail` 面向人工可读（进台账 hit_rules 与
/// 拒绝通知），证据脱敏在评估器内部完成（如 passkey 只报「含 40 位
/// 十六进制口令段」，不回显原文——策划案 §16）。
#[derive(Debug, Clone)]
pub struct RuleHit {
    pub rule: &'static str,
    /// deny / postpone / log（已套用站长配置的动作档）
    pub action: Action,
    /// 软规则贡献分（硬规则为 0）
    pub score: i32,
    pub detail: String,
}

/// 动作档（与 settings 键 `arule_*_action` 的 v 值同口径）。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Action {
    Deny,
    Postpone,
    Log,
}

impl Action {
    pub fn parse(v: &str) -> Action {
        match v {
            "deny" => Action::Deny,
            "postpone" => Action::Postpone,
            _ => Action::Log,
        }
    }
    pub fn as_str(&self) -> &'static str {
        match self {
            Action::Deny => "deny",
            Action::Postpone => "postpone",
            Action::Log => "log",
        }
    }
}

/// 引擎配置：worker 每轮从 site_settings 读一次（键清单见 0400 迁移；
/// 保存即生效——本仓业务读键实时 SQL 的既有口径）。
#[derive(Debug, Clone)]
pub struct EngineConfig {
    pub enabled: bool,
    /// shadow / half / full
    pub mode: String,
    pub delay_sec: i64,
    pub daily_cap: i64,
    pub descr_min_chars: usize,
    pub dupe_threshold_pct: i32,
    /// 命名检查力度：off / warn / strict
    pub title_level: String,
    pub title_template: String,
    /// 严格档必备要素（空 = 模板要素皆必备）
    pub title_required: Vec<String>,
    pub ad_words: Vec<String>,
    pub trusted_uploaders: Vec<i64>,
    pub watch_uploaders: Vec<i64>,
    /// 各规则 enabled/action（键 = 规则 id）
    pub rules: std::collections::HashMap<&'static str, (bool, Action)>,
}

impl EngineConfig {
    /// 出厂缺省（均衡号 + 影子）。真实配置由调用方覆盖。
    pub fn defaults() -> EngineConfig {
        let mut rules = std::collections::HashMap::new();
        for (id, on, a) in [
            ("arule_title_garbled", true, Action::Deny),
            ("arule_descr_missing", true, Action::Postpone),
            ("arule_ads_keywords", true, Action::Deny),
            ("arule_leak_marker", true, Action::Deny),
            ("arule_dupe_check", true, Action::Postpone),
            ("arule_title_format", true, Action::Postpone),
        ] {
            rules.insert(id, (on, a));
        }
        EngineConfig {
            enabled: false,
            mode: "shadow".into(),
            delay_sec: 90,
            daily_cap: 0,
            descr_min_chars: 80,
            dupe_threshold_pct: 92,
            title_level: "warn".into(),
            title_template: String::new(),
            title_required: Vec::new(),
            ad_words: Vec::new(),
            trusted_uploaders: Vec::new(),
            watch_uploaders: Vec::new(),
            rules,
        }
    }

    fn rule(&self, id: &'static str) -> (bool, Action) {
        self.rules.get(id).copied().unwrap_or((false, Action::Log))
    }
}

/// 评估输入：种子快照（发布时已有字段 + 文件清单摘要）。
#[derive(Debug, Clone, Default)]
pub struct TorrentSnapshot {
    pub torrent_id: i64,
    pub owner_id: i64,
    pub title: String,
    pub small_descr: String,
    /// 描述原文（含标签；去标签在评估器内做）
    pub descr: String,
    pub category_id: i64,
    pub site_type: String,
    /// 文件名清单（torrent_files；D 系深检用，H/S 轮可空）
    pub file_names: Vec<String>,
    /// 近期过审种标题（H5 相似度比对窗口；调用方控制窗口大小）
    pub recent_titles: Vec<String>,
}

/// 发布者画像（软规则用；按站型维度统计是取数侧的事，这里只收数）。
#[derive(Debug, Clone, Default)]
pub struct PublisherProfile {
    pub publish_credits: i32,
    pub approve_streak: i32,
    pub registered_days: i32,
    /// 近 90 天：人工通过数 / 被拒数
    pub approved_90d: i32,
    pub denied_90d: i32,
}

/// 评估总输出。
#[derive(Debug, Clone)]
pub struct Assessment {
    pub hits: Vec<RuleHit>,
    /// 软规则综合分（0-100，越高越可信）
    pub score: i32,
    /// 引擎建议：pass / deny / postpone（灰区=pass 不了也 deny 不了）
    pub suggestion: Suggestion,
    /// 命名引擎生成的建议标题（H6 命中时给纠错用；其余为 None）
    pub suggested_title: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Suggestion {
    Pass,
    Deny,
    Postpone,
}

/// 去标签：descr 里的 HTML 标签与实体展开后计数（空描述/纯图床链接
/// 的「纯标签描述」都会在这里露馅）。
pub fn strip_tags(input: &str) -> String {
    let mut out = String::with_capacity(input.len());
    let mut in_tag = false;
    let mut chars = input.chars().peekable();
    while let Some(c) = chars.next() {
        match c {
            '<' => in_tag = true,
            '>' => in_tag = false,
            '&' if !in_tag => {
                // 常见实体粗展开（nbsp 最常见——它算「空」不算「字」）
                let rest: String = std::iter::once('&')
                    .chain(chars.clone().take(6))
                    .collect();
                if rest.starts_with("&nbsp;") {
                    // "&nbsp;" = '&' + 5 个字符；多吞一个会把正文吃掉
                    for _ in 0..5 {
                        chars.next();
                    }
                    out.push(' ');
                } else {
                    out.push('&');
                }
            }
            c if !in_tag => out.push(c),
            _ => {}
        }
    }
    out
}

/// H1：标题乱码/夹码（策划案 §2.1）。
///
/// 三个特征任一命中：①替换符/控制符；②连续「非常见文字系统」字符占比
/// 过高（乱码的典型形态是 CJK 与私有区混杂）；③长度 ≥20 的 base64/
/// 十六进制串（夹码）。刻意保守——正常标题（中文/英文/数字/点线括号）
/// 不应触发任何一支。
pub fn hit_title_garbled(title: &str) -> Option<String> {
    let mut weird = 0usize;
    let mut total = 0usize;
    let mut prev_is_word_char = false;
    let mut run = 0usize;
    let mut max_run = 0usize;
    for c in title.chars() {
        total += 1;
        let is_word = c.is_ascii_alphanumeric();
        if c == '\u{FFFD}'
            || (c.is_control() && c != ' ')
            || matches!(c as u32, 0xE000..=0xF8FF)
        {
            weird += 1;
        }
        // base64/16 进制长串：字母数字连续 ≥20（文件名场景的合理 hex 段
        // 如 infohash 不会出现在「标题」里）
        if is_word {
            run += 1;
            max_run = max_run.max(run);
        } else {
            run = 0;
        }
        prev_is_word_char = is_word;
    }
    let _ = prev_is_word_char;
    if total == 0 {
        return None;
    }
    if weird * 10 >= total * 3 {
        return Some("标题含乱码/不可读字符".into());
    }
    if max_run >= 20 {
        return Some("标题夹藏长编码串（≥20 位连续字母数字）".into());
    }
    None
}

/// H2：描述不完整（去标签后过短）。PTGen 卡片不算入正文——快照里
/// descr 为发布者手写部分，模板卡片由前端另行注入，不进本字段。
pub fn hit_descr_missing(descr: &str, min_chars: usize) -> Option<String> {
    let text = strip_tags(descr);
    let trimmed = text.trim();
    let n = trimmed.chars().count();
    if n < min_chars {
        Some(format!(
            "描述内容过短（去标签后 {n} 字，要求 ≥{min_chars} 字）；请补充资源介绍/截图说明/MediaInfo"
        ))
    } else {
        None
    }
}

/// H3：广告/引流词库（包含匹配，大小写不敏感）。
pub fn hit_ads_keywords(text: &str, words: &[String]) -> Option<String> {
    let hay = text.to_lowercase();
    let mut hit: Vec<&str> = Vec::new();
    for w in words {
        let w = w.trim();
        if w.is_empty() {
            continue;
        }
        if hay.contains(&w.to_lowercase()) {
            hit.push(w);
        }
    }
    if hit.is_empty() {
        None
    } else {
        // §16：广告词给词（纠错需要）；单条通知最多列 5 个防刷屏
        let listed = hit.iter().take(5).cloned().collect::<Vec<_>>().join("、");
        Some(format!("描述命中广告/引流词：{listed}（共 {} 处）", hit.len()))
    }
}

/// H4：站内敏感信息（passkey 段 / tracker announce 明文）。
/// 证据脱敏：只报类型不回显内容（§16）。
pub fn hit_leak_marker(text: &str) -> Option<String> {
    let mut has_hex40 = false;
    let mut run = 0usize;
    for c in text.chars() {
        if c.is_ascii_hexdigit() {
            run += 1;
            if run == 40 {
                has_hex40 = true;
                break;
            }
        } else {
            run = 0;
        }
    }
    if has_hex40 {
        return Some("描述含 40 位十六进制串（疑似 passkey，已脱敏）".into());
    }
    let low = text.to_lowercase();
    for mark in ["/announce", "passkey="] {
        if low.contains(mark) {
            return Some(format!(
                "描述含 tracker 请求串（{mark}，疑似泄露 passkey 通道）"
            ));
        }
    }
    None
}

/// H5 辫子：标题相似度。先按非字母数字切 token、丢掉纯数字、单字母与
/// 技术词（分辨率/来源/编码词表，小写匹配），再对余下 token 的**字符
/// 集合**（非 bigram——组名/GROUPX 这类尾部噪声让 bigram 分母虚胖，
/// 同作不同版本反而被压低）算 Jaccard×100。量「作品名层」重合度：
/// 同作不同版本（WEB-DL vs BluRay）高分，不同作品低分。
pub fn title_similarity_pct(a: &str, b: &str) -> i32 {
    const TECH: &[&str] = &[
        "480p", "720p", "1080p", "1080i", "2160p", "4k", "8k",
        "bluray", "blu", "ray", "web", "webdl", "webrip", "hdtv",
        "remux", "dvdrip", "bdrip", "x264", "x265", "h264", "h265",
        "hevc", "avc", "av1", "dl", "hd", "sd",
    ];
    fn chars_of(s: &str) -> std::collections::HashSet<char> {
        let mut set = std::collections::HashSet::new();
        for t in s.split(|c: char| !c.is_alphanumeric()) {
            let l = t.to_lowercase();
            if l.chars().count() > 1
                && !t.chars().all(|c| c.is_ascii_digit())
                && !TECH.contains(&l.as_str())
            {
                set.extend(l.chars());
            }
        }
        set
    }
    let (sa, sb) = (chars_of(a), chars_of(b));
    if sa.is_empty() || sb.is_empty() {
        return 0;
    }
    let inter = sa.intersection(&sb).count();
    let union = sa.union(&sb).count();
    (inter as f64 * 100.0 / union as f64).round() as i32
}

/// H5：疑似重复（对窗口内标题逐一比相似度，超门槛即命中）。
pub fn hit_dupe(
    title: &str,
    recent: &[String],
    threshold_pct: i32,
) -> Option<String> {
    let mut best: Option<(i32, &str)> = None;
    for other in recent {
        let s = title_similarity_pct(title, other);
        if best.map_or(true, |(b, _)| s > b) {
            best = Some((s, other.as_str()));
        }
    }
    let Some((s, other)) = best else { return None };
    if s >= threshold_pct {
        Some(format!("与近期过审种相似度 {s}%：{other}"))
    } else {
        None
    }
}

/// 命名引擎（§2.6）：模板要素提取。内置词表；模板编译 = 占位符切分。
/// 返回 (命中的要素名列表, 缺失要素名列表, 建议标题)。
pub fn title_elements(
    title: &str,
    template: &str,
    required: &[String],
) -> (Vec<&'static str>, Vec<&'static str>, Option<String>) {
    // 内置词表（策划案 §2.6 十四要素的常用八项；站型特有要素在批 1.5）
    const RESOLUTIONS: &[&str] = &[
        "480p", "720p", "1080p", "1080i", "2160p", "4K", "4k", "8K",
    ];
    const SOURCES: &[&str] = &[
        "BluRay", "Blu-ray", "WEB-DL", "WEBRip", "HDTV", "Remux",
        "DVDRip", "BDRip", "WEB",
    ];
    const CODECS: &[&str] = &[
        "x264", "x265", "H.264", "H.265", "H264", "H265", "AVC", "HEVC",
        "AV1",
    ];
    let mut found: Vec<&'static str> = Vec::new();
    let mut year = None;
    let mut resolution = None;
    let mut source = None;
    let mut codec = None;
    let mut group = None;
    // 年份：19xx/20xx 且不是 4K/1080 等技术数（独立 token 判定）
    for tok in title.split(|c: char| !c.is_ascii_alphanumeric()) {
        if let Ok(y) = tok.parse::<i32>() {
            if (1900..=2099).contains(&y) && !tok.starts_with('0') {
                year = Some(y.to_string());
            }
        }
    }
    for r in RESOLUTIONS {
        if title.contains(r) {
            resolution = Some(*r);
            break;
        }
    }
    for s in SOURCES {
        if title.contains(s) {
            source = Some(*s);
            break;
        }
    }
    for c in CODECS {
        if title.contains(c) {
            codec = Some(*c);
            break;
        }
    }
    // 组名：末尾 -GROUP 段（PT 惯例）
    if let Some(pos) = title.rfind('-') {
        let tail = &title[pos + 1..];
        let tail_ok = !tail.is_empty()
            && tail.len() <= 24
            && tail
                .chars()
                .all(|c| c.is_ascii_alphanumeric() || c == '@');
        if tail_ok {
            group = Some(tail.to_string());
        }
    }
    // 中文名：最长 CJK 连续段
    let mut best_cjk = String::new();
    let mut cur = String::new();
    for c in title.chars() {
        if matches!(c as u32, 0x4E00..=0x9FFF) {
            cur.push(c);
        } else if cur.len() > best_cjk.len() {
            best_cjk = std::mem::take(&mut cur);
        } else {
            cur.clear();
        }
    }
    if cur.len() > best_cjk.len() {
        best_cjk = cur;
    }
    if year.is_some() {
        found.push("年份");
    }
    if resolution.is_some() {
        found.push("分辨率");
    }
    if source.is_some() {
        found.push("来源");
    }
    if codec.is_some() {
        found.push("编码");
    }
    if group.is_some() {
        found.push("组名");
    }
    if !best_cjk.is_empty() {
        found.push("中文名");
    }
    // 缺失 = 模板要求 - 已命中；required 覆盖模板要素集
    let wanted: Vec<&str> = if required.is_empty() {
        template
            .split(|c: char| c == '{' || c == '}')
            .filter(|s| {
                !s.is_empty()
                    && !s.chars().all(|c| {
                        "!@#$%^&*().,;:[] -".contains(c)
                    })
            })
            .map(|s| match s {
                "中文名" | "年份" | "分辨率" | "来源" | "编码" | "组名" => s,
                _ => "中文名", // 未识别占位符不参与缺失判定
            })
            .collect()
    } else {
        required.iter().map(|s| s.as_str()).collect()
    };
    let missing: Vec<&'static str> = wanted
        .into_iter()
        .filter(|w| !found.contains(w))
        .map(|w| match w {
            "中文名" => "中文名",
            "年份" => "年份",
            "分辨率" => "分辨率",
            "来源" => "来源",
            "编码" => "编码",
            "组名" => "组名",
            _ => "中文名",
        })
        .collect();
    // 建议标题：把已提取要素按模板重排（缺的跳过）
    let suggested = if template.is_empty() || missing.is_empty() {
        None
    } else {
        let mut s = template.to_string();
        for (ph, val) in [
            ("{中文名}", best_cjk.clone()),
            ("{年份}", year.clone().unwrap_or_default()),
            ("{分辨率}", resolution.unwrap_or("").to_string()),
            ("{来源}", source.unwrap_or("").to_string()),
            ("{编码}", codec.unwrap_or("").to_string()),
            ("{组名}", group.unwrap_or_default()),
        ] {
            s = s.replace(ph, &val);
        }
        // 清掉空占位符留下的「..」「--」
        while s.contains("..") {
            s = s.replace("..", ".");
        }
        while s.contains("--") {
            s = s.replace("--", "-");
        }
        Some(s.trim_matches(|c: char| !c.is_alphanumeric() && !matches!(c, '\u{4E00}'..='\u{9FFF}')).to_string())
    };
    (found, missing, suggested)
}

/// 软规则综合分（§2.2）：发布者可信度为主，元数据完整度微调。
/// 返回 0-100。评分刻意保守：观察名单直接压到暂缓线以下。
pub fn soft_score(t: &TorrentSnapshot, p: &PublisherProfile) -> i32 {
    let mut score = 0i32;
    // S1 发布者信用（封顶 60）
    score += (p.publish_credits * 5).min(40);
    score += (p.approve_streak * 2).min(12);
    score += (p.registered_days / 30).min(8);
    // S2 元数据完整度（副标题/正文长度/技术描述关键词三个便宜代理，封顶 20）
    if !t.small_descr.trim().is_empty() {
        score += 5;
    }
    let text = strip_tags(&t.descr);
    if text.chars().count() >= 100 {
        score += 7;
    }
    let low = text.to_lowercase();
    if ["mediainfo", "压制", "字幕", "音轨", "编码", "截图", "来源"]
        .iter()
        .any(|k| low.contains(k))
    {
        score += 8;
    }
    // S3 历史通过率（-20..+15）；近期被拒另计小惩罚
    let total = p.approved_90d + p.denied_90d;
    if total >= 5 {
        let rate = p.approved_90d * 100 / total.max(1);
        if rate >= 95 {
            score += 15;
        } else if rate < 60 {
            score -= 20;
        }
    }
    if p.denied_90d >= 2 {
        score -= 10;
    }
    score.clamp(0, 100)
}

/// 全量评估：H 规则短路（deny 档一票即出）→ 软评分 → 分数段建议。
/// pass_score 缺省 80（策划案 §2.2）；软规则不产生 deny（一期口径：
/// 自动拒绝必须硬规则证据）。
pub fn assess(t: &TorrentSnapshot, p: &PublisherProfile, cfg: &EngineConfig) -> Assessment {
    let mut hits: Vec<RuleHit> = Vec::new();
    let mut suggested_title = None;

    // 白名单：跳过一切 deny 档（仍可暂缓）；观察名单：不自动通过
    let trusted = cfg.trusted_uploaders.contains(&t.owner_id);
    let watched = cfg.watch_uploaders.contains(&t.owner_id);

    let text_all = format!("{}\n{}", t.title, strip_tags(&t.descr));

    // H1 乱码
    let (on, act) = cfg.rule("arule_title_garbled");
    if on {
        if let Some(d) = hit_title_garbled(&t.title) {
            hits.push(RuleHit { rule: "arule_title_garbled", action: if trusted { Action::Postpone } else { act }, score: 0, detail: d });
        }
    }
    // H2 描述
    let (on, act) = cfg.rule("arule_descr_missing");
    if on {
        if let Some(d) = hit_descr_missing(&t.descr, cfg.descr_min_chars) {
            hits.push(RuleHit { rule: "arule_descr_missing", action: if trusted { Action::Postpone } else { act }, score: 0, detail: d });
        }
    }
    // H3 广告
    let (on, act) = cfg.rule("arule_ads_keywords");
    if on && !cfg.ad_words.is_empty() {
        if let Some(d) = hit_ads_keywords(&text_all, &cfg.ad_words) {
            hits.push(RuleHit { rule: "arule_ads_keywords", action: if trusted { Action::Postpone } else { act }, score: 0, detail: d });
        }
    }
    // H4 泄露
    let (on, act) = cfg.rule("arule_leak_marker");
    if on {
        if let Some(d) = hit_leak_marker(&text_all) {
            hits.push(RuleHit { rule: "arule_leak_marker", action: if trusted { Action::Postpone } else { act }, score: 0, detail: d });
        }
    }
    // H5 重复
    let (on, act) = cfg.rule("arule_dupe_check");
    if on && !t.recent_titles.is_empty() {
        if let Some(d) = hit_dupe(&t.title, &t.recent_titles, cfg.dupe_threshold_pct) {
            hits.push(RuleHit { rule: "arule_dupe_check", action: if trusted { Action::Postpone } else { act }, score: 0, detail: d });
        }
    }
    // H6 命名（level=off 时整条跳过）
    let (on, act) = cfg.rule("arule_title_format");
    if on && cfg.title_level != "off" && !cfg.title_template.is_empty() {
        let (_, missing, suggested) =
            title_elements(&t.title, &cfg.title_template, &cfg.title_required);
        if !missing.is_empty() {
            suggested_title = suggested.clone();
            let list = missing.join("、");
            // strict 档缺必备要素才计为可拒动作；warn 档永远降为 postpone
            let eff = if cfg.title_level == "strict" && !trusted {
                act
            } else {
                Action::Postpone
            };
            hits.push(RuleHit {
                rule: "arule_title_format",
                action: eff,
                score: 0,
                detail: format!(
                    "标题缺必备要素：{list}{}",
                    suggested.map(|s| format!("；建议标题：{s}")).unwrap_or_default()
                ),
            });
        }
    }

    // 软评分
    let score = soft_score(t, p);
    // 建议：任何 deny 档命中 → Deny；否则分数 ≥80 且非观察名单 → Pass；
    // 其余 → Postpone（灰区给人工）
    let has_deny = hits.iter().any(|h| h.action == Action::Deny);
    let suggestion = if has_deny {
        Suggestion::Deny
    } else if score >= 80 && !watched {
        Suggestion::Pass
    } else {
        Suggestion::Postpone
    };
    Assessment { hits, score, suggestion, suggested_title }
}

/// 台账 hit_rules 的 JSONB 形态（0400 表注释口径）。
pub fn hits_to_json(hits: &[RuleHit]) -> serde_json::Value {
    serde_json::Value::Array(
        hits.iter()
            .map(|h| {
                json!({
                    "rule": h.rule,
                    "action": h.action.as_str(),
                    "score": h.score,
                    "detail": h.detail,
                })
            })
            .collect(),
    )
}


/// D2：可疑可执行文件（§2.3）。白名单分类由调用方在 file_names 之外
/// 判定（EngineConfig 无分类维度——分类豁免在 worker 取件侧做）。
pub fn hit_suspicious_exe(
    file_names: &[String],
    exe_whitelist: bool,
) -> Option<String> {
    if exe_whitelist {
        return None;
    }
    const BAD: &[&str] = &[".exe", ".scr", ".bat", ".cmd", ".apk", ".msi", ".vbs"];
    let hits: Vec<&str> = file_names
        .iter()
        .filter(|f| {
            let low = f.to_lowercase();
            BAD.iter().any(|e| low.ends_with(e))
        })
        .map(|f| f.as_str())
        .collect();
    if hits.is_empty() {
        None
    } else {
        let listed = hits.iter().take(3).cloned().collect::<Vec<_>>().join("、");
        Some(format!(
            "种内含可执行文件：{listed}（共 {} 个；如属工具类资源请选对应分类或加分类白名单）",
            hits.len()
        ))
    }
}

/// D4：文件数 × 命名话数交叉（§2.3）。标题 `[01-12]`/`01-12` 跨度应与
/// 视频文件数一致；字幕文件数应为视频数的 0..=1 倍。
pub fn hit_files_crosscheck(
    title: &str,
    file_names: &[String],
) -> Option<String> {
    // 跨度提取：E?01-12 / [01-12] / 第01-12话
    let mut span: Option<(i32, i32)> = None;
    let bytes: Vec<char> = title.chars().collect();
    let mut i = 0;
    while i + 4 <= bytes.len() {
        let c = bytes[i];
        let is_d = |x: char| x.is_ascii_digit();
        // 模式：d+ '-' d+
        if is_d(c) {
            let mut j = i;
            while j < bytes.len() && is_d(bytes[j]) {
                j += 1;
            }
            if j < bytes.len() && bytes[j] == '-' && j + 1 < bytes.len() && is_d(bytes[j + 1]) {
                let mut k = j + 1;
                while k < bytes.len() && is_d(bytes[k]) {
                    k += 1;
                }
                // 两侧不能也是数字字符（已被 while 吃掉），且左侧不是年份位
                let a: String = bytes[i..j].iter().collect();
                let b: String = bytes[j + 1..k].iter().collect();
                if let (Ok(a), Ok(b)) = (a.parse::<i32>(), b.parse::<i32>()) {
                    let len_a = (j - i) as i32;
                    let len_b = (k - j - 1) as i32;
                    if a >= 1 && b > a && b - a <= 200 && len_a >= 2 && len_b >= 2 {
                        span = Some((a, b));
                        i = k;
                        continue;
                    }
                }
            }
            i = j.max(i + 1);
        } else {
            i += 1;
        }
    }
    let Some((a, b)) = span else { return None };
    let expect = b - a + 1;
    let is_video = |f: &str| {
        let low = f.to_lowercase();
        [".mp4", ".mkv", ".avi", ".wmv", ".mov", ".flv", ".ts", ".m2ts"]
            .iter()
            .any(|e| low.ends_with(e))
    };
    let is_sub = |f: &str| {
        let low = f.to_lowercase();
        [".srt", ".ass", ".ssa", ".sub", ".idx", ".sup"]
            .iter()
            .any(|e| low.ends_with(e))
    };
    let videos = file_names.iter().filter(|f| is_video(f)).count() as i32;
    let subs = file_names.iter().filter(|f| is_sub(f)).count() as i32;
    if videos == 0 {
        return None; // 非视频包（音乐/书籍）不适用
    }
    if videos != expect {
        return Some(format!(
            "标题话数跨度 [{a:02}-{b:02}] 应有 {expect} 个视频文件，实际 {videos} 个"
        ));
    }
    if subs > videos * 2 {
        return Some(format!(
            "字幕文件 {subs} 个远超视频数 {videos}，请检查打包"
        ));
    }
    None
}

/// D5（本地部分）：文件声称 vs 内容清单粗校验——声称视频的种子最大文件
/// 应是视频容器（magic number 校验在批 4 沙箱深检；这里是文件名层近似）。
pub fn hit_fake_media_by_names(file_names: &[String]) -> Option<String> {
    let mut max_name: Option<&String> = None;
    let mut max_len = 0usize;
    for f in file_names {
        if f.len() > max_len {
            max_len = f.len();
            max_name = Some(f);
        }
    }
    let Some(big) = max_name else { return None };
    let low = big.to_lowercase();
    let known_media = [".mp4", ".mkv", ".avi", ".iso", ".ts", ".m2ts", ".flac", ".ape", ".wav", ".mp3"]
        .iter()
        .any(|e| low.ends_with(e));
    let looks_media = low.contains("sample") == false && known_media;
    if !looks_media {
        return Some(format!(
            "最大文件不是常见媒体容器：{big}（如为正片请检查打包）"
        ));
    }
    None
}


/// D3（§2.3）：伪媒体交叉——标题/文件声称的容器 vs MediaInfo 解析出的
/// 实际容器族。真源=torrents.mediainfo_check.summary（0390 发布时已解析，
/// 零新增 IO）。`mi_container` 形如 "MKV"/"MP4"/"FLAC"（MediaInfo
/// General 段的 Format）。解析缺位（None）→ 无法判定 → 不命中（宁可
/// 漏放不可误杀，策划案 §1.2 原则 3）。
pub fn hit_fake_media_cross(
    mi_container: Option<&str>,
    file_names: &[String],
) -> Option<String> {
    let Some(mi) = mi_container else {
        return None;
    };
    let mi_up = mi.to_ascii_uppercase();
    // 文件层声称：最大文件扩展名
    let biggest = file_names
        .iter()
        .max_by_key(|f| f.len())
        .map(|f| f.as_str())
        .unwrap_or("");
    let ext = biggest
        .rsplit('.')
        .next()
        .map(|e| e.to_ascii_uppercase())
        .unwrap_or_default();
    // 同族判定表：容器扩展名 ↔ MediaInfo Format 名（常见对）
    const FAMILY: &[(&str, &[&str])] = &[
        ("MKV", &["MKV", "MATROSKA", "WEBM"]),
        ("MP4", &["MP4", "MPEG-4", "MOV", "M4V"]),
        ("AVI", &["AVI"]),
        ("TS", &["MPEG-TS", "TS", "M2TS"]),
        ("FLAC", &["FLAC"]),
        ("APE", &["APE", "MONKEY"]),
        ("WAV", &["WAVE", "WAV"]),
        ("MP3", &["MPEG AUDIO", "MP3"]),
        ("ISO", &["ISO"]),
    ];
    if ext.is_empty() || ext == biggest.to_ascii_uppercase() {
        return None; // 最大文件无扩展名——交给文件名层 D5
    }
    for (e, family) in FAMILY {
        if *e == ext && !family.contains(&mi_up.as_str()) {
            return Some(format!(
                "最大文件扩展名 .{ext} 与 MediaInfo 容器 {mi} 不符（伪媒体嫌疑，请人工核验）"
            ));
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    fn snap(title: &str, descr: &str) -> TorrentSnapshot {
        TorrentSnapshot {
            torrent_id: 1,
            owner_id: 10,
            title: title.into(),
            small_descr: "副标题".into(),
            descr: descr.into(),
            category_id: 1,
            site_type: "general".into(),
            file_names: vec![],
            recent_titles: vec![],
        }
    }

    fn cfg() -> EngineConfig {
        let mut c = EngineConfig::defaults();
        c.ad_words = vec!["微信群".into(), "t.me/".into()];
        c.title_template = "{中文名}.{年份}.{来源}.{分辨率}-{组名}".into();
        c
    }

    const LONG_DESCR: &str = "这是一份足够长的资源描述，包含来源说明、压制参数、字幕信息与观看建议，\
        全文超过一百字符以满足 H2 的下限与软规则 S2 的长度档要求。\
        补充第二段介绍音轨与章节信息，以及制作花絮和字幕组的说明，\
        再补一段给收藏党看的版本对比：本版相对旧版修正了部分译名，\
        并附完整中文特效字幕，适合首次收藏。";

    #[test]
    fn golden_corpus_sixty_plus() {
        // §18.3 金标准回归集：60+ 例数据表在 auto_review_corpus.rs。
        // 改规则后此表 diff 即「基线外变化」，须人工确认再改期望。
        let n = super::super::auto_review_corpus::run_corpus(&cfg());
        assert!(n >= 60);
    }

    #[test]
    fn h1_garbled_detects_replacement_and_code() {
        assert!(hit_title_garbled("资源\u{FFFD}\u{FFFD}\u{FFFD}\u{FFFD}\u{FFFD}\u{FFFD}").is_some());
        assert!(hit_title_garbled("标题 aBcDeFgHiJkLmNoPqRsTv").is_some());
        assert!(hit_title_garbled("流浪地球2.2023.WEB-DL.2160p.H265-GROUP").is_none());
        assert!(hit_title_garbled("进击的巨人 第01-12话").is_none());
    }

    #[test]
    fn h2_descr_length_after_tag_strip() {
        assert!(hit_descr_missing("<img src=x><br/><br/>", 80).is_some());
        assert!(hit_descr_missing(LONG_DESCR, 80).is_none());
        assert_eq!(strip_tags("a&nbsp;&nbsp;b"), "a  b");
    }

    #[test]
    fn h3_ads_hits_words_case_insensitive() {
        let d = LONG_DESCR.to_string() + " 进群看 T.ME/abc123";
        assert!(hit_ads_keywords(&d, &["t.me/".into(), "微信群".into()]).is_some());
        assert!(hit_ads_keywords(LONG_DESCR, &["微信群".into()]).is_none());
    }

    #[test]
    fn h4_leak_marker_masks_evidence() {
        let d = format!("{LONG_DESCR} passkey: {}", "a".repeat(40));
        let hit = hit_leak_marker(&d).unwrap();
        assert!(hit.contains("已脱敏"));
        assert!(!hit.contains("aaaaaaaa"));
        assert!(hit_leak_marker(LONG_DESCR).is_none());
    }

    #[test]
    fn h5_similarity_bigram_ladder() {
        let a = "流浪地球2.2023.WEB-DL.2160p.H265-GROUP";
        let same = "流浪地球2.2023.BluRay.1080p.x264-OTHER";
        let other = "进击的巨人.2023.WEB-DL.1080p.x264-GRP";
        assert!(title_similarity_pct(a, same) > title_similarity_pct(a, other));
        let mut recent = vec![same.to_string()];
        assert!(hit_dupe(a, &recent, 40).is_some());
        recent = vec![other.to_string()];
        assert!(hit_dupe(a, &recent, 92).is_none());
    }

    #[test]
    fn h6_template_extract_and_suggest() {
        let ok = "流浪地球2.2023.WEB-DL.2160p.H265-GROUP";
        let (found, missing, _) = title_elements(ok, "{中文名}.{年份}.{来源}.{分辨率}-{组名}", &[]);
        assert!(missing.is_empty(), "missing={missing:?} found={found:?}");
        let bad = "流浪地球2 WEB-DL GROUPX";
        let (_, missing, suggested) = title_elements(bad, "{中文名}.{年份}.{来源}.{分辨率}-{组名}", &[]);
        assert!(missing.contains(&"年份") && missing.contains(&"分辨率"));
        let s = suggested.unwrap();
        assert!(s.contains("2023") || s.contains("流浪地球"));
    }


    #[test]
    fn d2_suspicious_exe_with_whitelist() {
        let files = vec!["setup.exe".to_string(), "readme.txt".to_string()];
        assert!(hit_suspicious_exe(&files, false).is_some());
        assert!(hit_suspicious_exe(&files, true).is_none());
        let clean = vec!["movie.mkv".to_string()];
        assert!(hit_suspicious_exe(&clean, false).is_none());
    }

    #[test]
    fn d4_span_crosscheck() {
        let mk = |n: i32| (0..n).map(|i| format!("ep{:02}.mkv", i + 1)).collect::<Vec<_>>();
        let ok = mk(12);
        assert!(hit_files_crosscheck("某剧 [01-12] 1080p", &ok).is_none());
        let short = mk(10);
        assert!(hit_files_crosscheck("某剧 [01-12] 1080p", &short).is_some());
        // 年份不应被当跨度（2023 不满足 len>=2 且 b>a？2023-? 常见无对）
        assert!(hit_files_crosscheck("电影.2023.WEB-DL", &mk(1)).is_none());
    }

    #[test]
    fn d5_fake_media_by_names() {
        let bad = vec!["cover.jpg".to_string(), "setup.bin".to_string()];
        assert!(hit_fake_media_by_names(&bad).is_some());
        let good = vec!["feature.mkv".to_string(), "sample.mkv".to_string()];
        assert!(hit_fake_media_by_names(&good).is_none());
    }
    #[test]
    fn assess_full_pipeline_three_ways() {
        // 干净高分 → Pass
        let mut p = PublisherProfile {
            publish_credits: 5,
            approve_streak: 6,
            registered_days: 400,
            approved_90d: 20,
            denied_90d: 0,
        };
        let a = assess(&snap("流浪地球2.2023.WEB-DL.2160p.H265-GROUP", LONG_DESCR), &p, &cfg());
        assert_eq!(a.suggestion, Suggestion::Pass, "hits={:?} score={}", a.hits, a.score);
        // 命中广告 deny 档 → Deny
        let d = format!("{LONG_DESCR} 加微信群获取");
        let a = assess(&snap("流浪地球2.2023.WEB-DL.2160p.H265-GROUP", &d), &p, &cfg());
        assert_eq!(a.suggestion, Suggestion::Deny);
        // 白名单压 deny → 高分照样 Pass（白名单语义=跳过自动拒绝，§3.1 卡片 4）
        let mut c = cfg();
        c.trusted_uploaders = vec![10];
        let a = assess(&snap("流浪地球2.2023.WEB-DL.2160p.H265-GROUP", &d), &p, &c);
        assert_eq!(a.suggestion, Suggestion::Pass, "hits={:?}", a.hits);
        assert!(a.hits.iter().all(|h| h.action != Action::Deny));
        // 低分新号灰区 → Postpone
        p = PublisherProfile::default();
        let a = assess(&snap("流浪地球2.2023.WEB-DL.2160p.H265-GROUP", LONG_DESCR), &p, &cfg());
        assert_eq!(a.suggestion, Suggestion::Postpone);
        // 观察名单不自动通过
        let p2 = PublisherProfile {
            publish_credits: 5,
            approve_streak: 6,
            registered_days: 400,
            approved_90d: 20,
            denied_90d: 0,
        };
        let mut c2 = cfg();
        c2.watch_uploaders = vec![10];
        let a = assess(&snap("流浪地球2.2023.WEB-DL.2160p.H265-GROUP", LONG_DESCR), &p2, &c2);
        assert_eq!(a.suggestion, Suggestion::Postpone);
    }
}
