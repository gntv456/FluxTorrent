//! MediaInfo 解析与一致性比对（评估报告 §1.5，防标题党）。
//!
//! 【要解决的问题】PT 站最常见的内容质量问题：标题写「2160p HDR DV」
//! 而实际文件是 1080p SDR，或写「x264」而实际是 x265。纯靠人工举报
//! 成本高、滞后久。这里做的是**上传时机器比对**：把 MediaInfo 文本
//! 解析成规格摘要，再与用户声明的标题/字段比对，不一致就标出来。
//!
//! 【为什么解析器写在 D 层】MediaInfo 的输出格式是纯文本 → 纯结构，
//! 不碰数据库也不碰 actix；比对判据同理。SQL 在 `repo/`。
//!
//! 【刻意不做的事】不因不一致就拒绝发布。判定是**提示**而非闸门：
//! 一是解析器对少见格式会失准（把合法片源误判成标题党，比放过几个
//! 标题党更伤用户信任）；二是真有问题的种子可以事后下架。

/// 从 MediaInfo 文本里解析出的规格摘要。
#[derive(Debug, Clone, Default, PartialEq, Eq, serde::Serialize)]
pub struct Summary {
    /// 宽×高，如 (1920, 1080)
    pub resolution: Option<(u32, u32)>,
    /// 视频编码，小写：h264 / hevc / av1 / vc1 / mpeg2 ...
    pub video_codec: Option<String>,
    /// 像素格式（位深线索）：yuv420p10le 等
    pub pixel_format: Option<String>,
    /// 是否 HDR
    pub hdr: bool,
    /// HDR 格式（若能识别）：dolby_vision / hdr10 / hlg
    pub hdr_format: Option<String>,
    /// 音频编码，如 ddp / ac3 / truehd / atmos
    pub audio_codec: Option<String>,
    /// 声道数
    pub audio_channels: Option<u32>,
    /// 位深（从 pixel format 或 bit depth 字段推出）
    pub bit_depth: Option<u32>,
}

/// 用户在发布时**声明**的规格（来自标题与自定义字段）。
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Declared {
    /// 标题里声明的分辨率，如 "1080p" / "2160p" / "4K"
    pub resolution: Option<String>,
    /// 声明的视频编码，如 "x264" / "x265" / "H.264"
    pub video_codec: Option<String>,
    /// 声明是否 HDR
    pub hdr: Option<bool>,
}

/// 一处不一致。
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
pub struct Mismatch {
    /// 字段名：resolution / video_codec / hdr
    pub field: String,
    /// 用户声明的值
    pub declared: String,
    /// MediaInfo 实际解析出的值
    pub actual: String,
    /// 人可读的解释
    pub why: String,
}

/// 比对结果。
#[derive(Debug, Clone, Default, PartialEq, Eq, serde::Serialize)]
pub struct CompareResult {
    pub mismatches: Vec<Mismatch>,
    /// 有多少字段声明了但 MediaInfo 里解析不出（无法判定，不算不一致）
    pub unverifiable: Vec<String>,
}

impl CompareResult {
    /// 是否确有标题党（严格：无法判定的字段不算）。
    pub fn is_mismatch(&self) -> bool {
        !self.mismatches.is_empty()
    }
}

/// 解析 MediaInfo 文本（典型为 `mediainfo --Full` 的输出）。
///
/// **容错优先**：MediaInfo 各版本/各平台的字段名与顺序都不同，
/// 且用户常只贴片段。解析不出来就留空——上层据此判「无法判定」，
/// 而不是编一个值出来。
pub fn parse(text: &str) -> Summary {
    let low = text.to_ascii_lowercase();
    let mut s = Summary::default();

    // 分辨率：优先 "1920 x 1080"，退回 Height/Width 分行
    if let Some((w, h)) = scan_resolution(&low) {
        s.resolution = Some((w, h));
    } else {
        let h = scan_stream_u32(&low, "video", "height");
        let w = scan_stream_u32(&low, "video", "width");
        if let (Some(w), Some(h)) = (w, h) {
            s.resolution = Some((w, h));
        }
    }

    // 视频编码：Format 紧跟在 Video 流里。取第一个 video format，
    // 避免误取音频的 Format。
    if let Some(f) = scan_stream_format(&low, "video") {
        s.video_codec = normalize_video_codec(&f);
    }
    s.pixel_format = scan_stream_kv(&low, "video", "pixel format");
    s.bit_depth = s.pixel_format.as_deref().and_then(bit_depth_of);

    // HDR：MediaInfo 用 Transfer characteristic / Color space
    // 限定在视频流内取：Complete name 那段也可能带同名字段。
    // MediaInfo 的键是复数 "Transfer characteristics"（不是 singular），
    // 两个拼法都收：不同版本/不同工具的输出不一致。
    let transfer = scan_stream_kv(&low, "video", "transfer characteristics")
        .or_else(|| scan_stream_kv(&low, "video", "transfer characteristic"))
        .unwrap_or_default();
    let color_space =
        scan_stream_kv(&low, "video", "color space").unwrap_or_default();
    // 实际值形如 "SMPTE ST 2084 (PQ)"（小写后含 "st 2084"）。
    if transfer.contains("smpte st 2084") || transfer.contains("pq") {
        s.hdr = true;
        s.hdr_format = Some("hdr10".into());
    } else if transfer.contains("arib-std-b67") || transfer.contains("hlg") {
        s.hdr = true;
        s.hdr_format = Some("hlg".into());
    }
    if color_space.contains("bt2020") && !s.hdr {
        // BT.2020 但 transfer 说不出 HDR：算 HDR 但格式未知
        s.hdr = true;
    }
    if let Some(f) = scan_dolby_vision(&low) {
        s.hdr = true;
        s.hdr_format = Some(f);
    }

    // 音频：第一个 audio 流
    if let Some(f) = scan_stream_format(&low, "audio") {
        s.audio_codec = normalize_audio_codec(&f);
        s.audio_channels = scan_stream_u32(&low, "audio", "channel(s)")
            .or_else(|| scan_stream_channel_layout(&low));
    }
    s
}

/// 在目标流内取某个数值字段。
/// 一行是否开始一个流记录块，并判定它是什么类型。
///
/// MediaInfo 有两套常见输出，原扫描器只认第一套，
/// 第二套整个地漏判（P-347 实测：现代版输出的 `Video` /
/// `ID : Video 1` 格式下分辨率与编码全部抽空，
/// “假 4K”这类最典型标题党恰好从缏程地漏过）：
///
/// 1. 旧格式：`Stream #0:0(eng): Video: AVC`
/// 2. 现代格式：独立的 `Video` / `Audio` 段落，段内用 `ID : Video 1` 标识
///
/// 返回 `Some(kind)` 表示该行开始了一个 `kind` 类型的流
/// （`"video"` / `"audio"`），调用方应 `continue` 跳过该行。
fn stream_kind_of(l: &str) -> Option<&'static str> {
    let head = l.to_ascii_lowercase();
    // 旧格式：`Stream #0:0: Video: AVC`
    if head.starts_with("stream ") {
        return if head.contains("video") {
            Some("video")
        } else if head.contains("audio") {
            Some("audio")
        } else {
            Some("")
        };
    }
    let bare = head.trim();
    if bare == "video" {
        return Some("video");
    }
    if bare == "audio" {
        return Some("audio");
    }
    // 现代格式的流标识行：`ID : Video 1`
    if bare.starts_with("id") {
        if let Some((_k, v)) = l.split_once(':') {
            let v = v.trim().to_ascii_lowercase();
            if v.starts_with("video") {
                return Some("video");
            }
            if v.starts_with("audio") {
                return Some("audio");
            }
        }
    }
    None
}

/// MediaInfo 的数字常带千分位空格（`1 920 pixels`）或单位
/// 后缀（`1920 pixels`、`23.976 FPS`）。直接 `parse()` 会全部失败，
/// 也是 P-347 的漏判之一。先取出数字部分再解析。
fn parse_num(v: &str) -> Option<u32> {
    let digits: String = v.chars().filter(|c| c.is_ascii_digit()).collect();
    if digits.is_empty() {
        return None;
    }
    digits.parse::<u32>().ok()
}

fn scan_stream_u32(low: &str, kind: &str, key: &str) -> Option<u32> {
    let mut in_stream = false;
    for line in low.lines() {
        let l = line.trim();
        if let Some(sk) = stream_kind_of(l) {
            in_stream = sk == kind;
            continue;
        }
        if in_stream {
            if let Some((k, v)) = l.split_once(':') {
                if k.trim() == key {
                    return parse_num(v.trim());
                }
            }
        }
    }
    None
}

/// 在音频流内由 `Channel layout : 5.1` 推出声道数。
fn scan_stream_channel_layout(low: &str) -> Option<u32> {
    let mut in_stream = false;
    for line in low.lines() {
        let l = line.trim();
        if let Some(sk) = stream_kind_of(l) {
            in_stream = sk == "audio";
            continue;
        }
        if in_stream {
            if let Some((k, v)) = l.split_once(':') {
                if k.trim() == "channel layout" {
                    let layout = v.trim();
                    let mut parts = layout.split('.');
                    let front: u32 =
                        parts.next().unwrap_or("").trim().parse().ok()?;
                    let back: u32 =
                        parts.next().unwrap_or("0").trim().parse().unwrap_or(0);
                    // LFE 记在布局尾部的 "L"（如 5.1(side) 无 L，5.1 视为含 LFE）
                    let lfe = if layout.contains("L") { 1 } else { 0 };
                    return Some(front + back + lfe);
                }
            }
        }
    }
    None
}

/// 在目标流内取字符串字段。
fn scan_stream_kv(low: &str, kind: &str, key: &str) -> Option<String> {
    let mut in_stream = false;
    for line in low.lines() {
        let l = line.trim();
        if let Some(sk) = stream_kind_of(l) {
            in_stream = sk == kind;
            continue;
        }
        if in_stream {
            if let Some((k, v)) = l.split_once(':') {
                if k.trim() == key {
                    let v = v.trim();
                    if !v.is_empty() {
                        return Some(v.to_string());
                    }
                }
            }
        }
    }
    None
}

/// `1920 x 1080` 形态。
fn scan_resolution(low: &str) -> Option<(u32, u32)> {
    let mut it = low.match_indices(" x ");
    for (i, _) in it.by_ref() {
        let left = low[..i].split_whitespace().last()?;
        let right_start = i + 3;
        let right: String = low[right_start..]
            .chars()
            .take_while(|c| c.is_ascii_digit())
            .collect();
        if let (Ok(w), Ok(h)) = (left.parse::<u32>(), right.parse::<u32>()) {
            if w > 0 && h > 0 && w < 100_000 && h < 100_000 {
                return Some((w, h));
            }
        }
    }
    None
}

/// `height    : 1080` 形态（冒号两侧可有空格）。
fn scan_kv_u32(low: &str, key: &str) -> Option<u32> {
    scan_kv(low, key)?.trim().parse().ok()
}

fn scan_kv(low: &str, key: &str) -> Option<String> {
    for line in low.lines() {
        if let Some((l, r)) = line.split_once(':') {
            if l.trim() == key {
                let v = r.trim();
                if !v.is_empty() {
                    return Some(v.to_string());
                }
            }
        }
    }
    None
}

/// 取某个流（video/audio）里的 `Format` 值。
///
/// MediaInfo 的结构是 `Stream #0:0... Video: ...` 之后若干 `Key: Value`
/// 行。故先定位流头行，再在下一个流头之前找 Format。
fn scan_stream_format(low: &str, kind: &str) -> Option<String> {
    let mut in_stream = false;
    for line in low.lines() {
        let l = line.trim();
        if let Some(sk) = stream_kind_of(l) {
            in_stream = sk == kind;
            continue;
        }
        if in_stream {
            if let Some((k, v)) = l.split_once(':') {
                if k.trim() == "format" {
                    let v = v.trim();
                    if !v.is_empty() {
                        return Some(v.to_string());
                    }
                }
            }
        }
    }
    None
}

fn scan_channel_count(low: &str) -> Option<u32> {
    // "Channel layout : 5.1" → 6
    let layout = scan_kv(low, "channel layout")?;
    let mut parts = layout.split('.');
    let front: u32 = parts.next()?.trim().parse().ok()?;
    let back: u32 = parts.next().unwrap_or("0").trim().parse().unwrap_or(0);
    let lfe = if layout.to_lowercase().contains('l') {
        1
    } else {
        0
    };
    Some(front + back + lfe)
}

/// Dolby Vision 的兼容层 / profile 标记。
fn scan_dolby_vision(low: &str) -> Option<String> {
    // Dolby Vision 的判定锚点是 `Dolby Vision ELG`（兼容层号）这个
    // MediaInfo 字段；只要它出现就是 DV。早前版本用 `?` 卡在
    // `scan_kv("dovi elg")` 上——键名拼错导致整个函数恒返回 None。
    let has_dovi_field = low.lines().any(|l| {
        l.split_once(':').is_some_and(|(k, _)| {
            k.trim().eq_ignore_ascii_case("dovi elg")
                || k.trim().eq_ignore_ascii_case("dovi elg")
        })
    });
    if has_dovi_field || low.contains("dolby vision") {
        Some("dolby_vision".into())
    } else {
        None
    }
}

/// 从像素格式推位深：yuv420p10le → 10、yuv420p → 8、yuv444p12 → 12。
///
/// **不能把全部数字连起来读**：`yuv420p10le` 里既有 `420`（色度采样）
/// 又有 `10`（位深），连起来会得到 42010。这里取 `p` 之后、`l`/`le`
/// 之前的那一段——那才是位深。
fn bit_depth_of(pf: &str) -> Option<u32> {
    let p = pf.find('p')?;
    let tail = &pf[p + 1..];
    let digits: String =
        tail.chars().take_while(|c| c.is_ascii_digit()).collect();
    if digits.is_empty() {
        // `yuv420p` / `nv12` 这类没有位深后缀 = 8 bit（SDR 的默认值）。
        // 返回 None 会让「解析不出位深」被上层判为无法判定，
        // 于是本可判定的片源被放过——宁可给出 8 也不要装不知道。
        if pf.contains('p') {
            return Some(8);
        }
        return None;
    }
    let d: u32 = digits.parse().ok()?;
    // 合理范围：8/10/12/14/16。超出说明格式不认识，别硬报。
    if (8..=16).contains(&d) {
        Some(d)
    } else {
        None
    }
}

fn normalize_video_codec(f: &str) -> Option<String> {
    let f = f.to_ascii_lowercase();
    let v = if f.contains("avc") || f.contains("h.264") || f.contains("h264") {
        "h264"
    } else if f.contains("hevc") || f.contains("h.265") || f.contains("h265") {
        "hevc"
    } else if f.contains("av1") {
        "av1"
    } else if f.contains("vc-1") || f.contains("vc1") {
        "vc1"
    } else if f.contains("mpeg-2") || f.contains("mpeg2") {
        "mpeg2"
    } else if f.contains("mpeg-4") || f.contains("mpeg4") || f.contains("divx")
    {
        "mpeg4"
    } else if f.contains("prores") {
        "prores"
    } else {
        return None;
    };
    Some(v.into())
}

fn normalize_audio_codec(f: &str) -> Option<String> {
    let f = f.to_ascii_lowercase();
    let v = if f.contains("ddp") || f.contains("eac3") || f.contains("e-ac-3") {
        "ddp"
    } else if f.contains("truehd") || f.contains("mlp") {
        "truehd"
    } else if f.contains("atmos") {
        "atmos"
    } else if f.contains("ac-3") || f.contains("ac3") {
        "ac3"
    } else if f.contains("dts") && f.contains("x") {
        "dtsx"
    } else if f.contains("dts") {
        "dts"
    } else if f.contains("flac") {
        "flac"
    } else if f.contains("aac") {
        "aac"
    } else if f.contains("opus") {
        "opus"
    } else if f.contains("pcm") {
        "pcm"
    } else {
        return None;
    };
    Some(v.into())
}

/// 声明的分辨率字符串 → (宽, 高)。支持 `1080p` / `2160p` / `4K` / `720P`。
pub fn parse_declared_resolution(s: &str) -> Option<(u32, u32)> {
    let t = s.trim().to_ascii_lowercase();
    match t.as_str() {
        "8k" | "4320p" => Some((7680, 4320)),
        "4k" | "2160p" | "uhd" => Some((3840, 2160)),
        "1440p" | "2k" => Some((2560, 1440)),
        "1080p" | "fullhd" | "fhd" => Some((1920, 1080)),
        "720p" | "hd" => Some((1280, 720)),
        "480p" | "sd" => Some((854, 480)),
        _ => {
            // 允许 "1920x1080" 形态
            if let Some((w, h)) = scan_resolution(&t) {
                Some((w, h))
            } else {
                None
            }
        }
    }
}

/// 声明的编码名 → 规范名。允许 x264/x.264/avc/h264 等写法。
pub fn normalize_declared_codec(s: &str) -> Option<String> {
    let t = s.trim().to_ascii_lowercase();
    // 不做任何前缀裁剪：'x.265' / 'h.265' / 'x264' 的分隔符位置不固定，
    // 统一按子串判定（先把 '.' 与 '-' 之外的写法归一）。
    let t: String = t.chars().filter(|c| *c != '.' && *c != '_').collect();
    if t.contains("x265")
        || t.contains("h265")
        || t.contains("h.265")
        || t.contains("hevc")
    {
        Some("hevc".into())
    } else if t.contains("x264")
        || t.contains("h264")
        || t.contains("h.264")
        || t.contains("avc")
    {
        Some("h264".into())
    } else if t.contains("av1") {
        Some("av1".into())
    } else if t.contains("xvid") || t.contains("divx") {
        Some("mpeg4".into())
    } else {
        None
    }
}

/// 一致性比对（评估报告 §1.5 的 `compare`）。
///
/// **只判「确凿」的不一致**：分辨率低于声明的（假 4K/假 1080p）、
/// 声明 HDR 但实际 SDR。声明 4K 而实际 2160p 是吻合的，不报。
pub fn compare(declared: &Declared, actual: &Summary) -> CompareResult {
    let mut r = CompareResult::default();

    // 分辨率
    if let Some(ds) = declared.resolution.as_deref() {
        match parse_declared_resolution(ds) {
            Some((dw, dh)) => match actual.resolution {
                Some((aw, ah)) => {
                    // 允许少量误差？不允许——标题写 1080p 而实际 1440p
                    // 是「低报」（少见但存在），这里按高报判（即实际
                    // 分辨率低于声明）。
                    if ah < dh || aw < dw {
                        r.mismatches.push(Mismatch {
                            field: "resolution".into(),
                            declared: format!("{dw}x{dh}"),
                            actual: format!("{aw}x{ah}"),
                            why: "实际分辨率低于标题声明".into(),
                        });
                    }
                }
                None => r.unverifiable.push("resolution".into()),
            },
            None => r.unverifiable.push("resolution".into()),
        }
    }

    // 视频编码
    if let Some(dc) = declared.video_codec.as_deref() {
        match normalize_declared_codec(dc) {
            Some(d) => match actual.video_codec.as_deref() {
                Some(a) if a == d => {}
                Some(a) => {
                    // h264 容器里装 av1 这类不现实；只报明确的跨族差异
                    let same_family = (a == "h264" && d == "mpeg4")
                        || (a == "mpeg4" && d == "h264");
                    if !same_family {
                        r.mismatches.push(Mismatch {
                            field: "video_codec".into(),
                            declared: d,
                            actual: a.into(),
                            why: "实际编码与标题声明不一致".into(),
                        });
                    }
                }
                None => r.unverifiable.push("video_codec".into()),
            },
            None => r.unverifiable.push("video_codec".into()),
        }
    }

    // HDR：只有「声明 HDR 但实际 SDR」才算标题党；
    // 「声明 SDR 但实际 HDR」不是问题（顶多是没写全）。
    if declared.hdr == Some(true) && !actual.hdr {
        // 若 MediaInfo 压根没给 transfer/color 信息，无法判定
        let has_color_info =
            actual.hdr_format.is_some() || actual.pixel_format.is_some();
        if has_color_info {
            r.mismatches.push(Mismatch {
                field: "hdr".into(),
                declared: "HDR".into(),
                actual: "SDR".into(),
                why: "标题声明 HDR 但 MediaInfo 显示为 SDR".into(),
            });
        } else {
            r.unverifiable.push("hdr".into());
        }
    }
    r
}

/// 脱敏 MediaInfo 文本（去路径里的用户名 / UNC 主机名 / 绝对路径）。
///
/// 报告 §1.5 特别点名：MediaInfo 的 `Complete name` 会泄露站长目录
/// 与用户名。这类信息进了公开详情页等于泄露内部结构。
pub fn sanitize(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    // lines() 会丢掉末尾换行。用 ends_with 判定并补回，
    // 否则「原样返回」的期望在有尾换行的输入上不成立。
    let trailing_nl = text.ends_with('\n');
    for line in text.lines() {
        if let Some((k, v)) = line.split_once(':') {
            let key = k.trim();
            if key.eq_ignore_ascii_case("complete name")
                || key.eq_ignore_ascii_case("complete name (location)")
                || key.eq_ignore_ascii_case("file name")
                || key.eq_ignore_ascii_case("file name (location)")
            {
                // 只保留文件名，不留目录
                let fname = v
                    .rsplit(['/', '\\'])
                    .find(|s| !s.is_empty())
                    .unwrap_or("(file)")
                    .trim();
                out.push_str(&format!("{key} : {}\n", mask_path(fname)));
                continue;
            }
        }
        // UNC 主机名：\\SERVER\share\... → \\***\share\...
        out.push_str(&mask_unc(line));
        out.push('\n');
    }
    if !trailing_nl {
        out.pop();
    }
    out
}

fn mask_unc(line: &str) -> String {
    let Some(rest) = line.strip_prefix("\\\\") else {
        return line.to_string();
    };
    let (host, tail) = match rest.find('\\') {
        Some(i) => (&rest[..i], &rest[i..]),
        None => (rest, ""),
    };
    format!("\\\\***{tail}")
}

/// 文件名里常见用户名/站点信息也要压掉：`user_release.mkv` → 保留扩展名。
fn mask_path(fname: &str) -> String {
    // 保留扩展名即可——目录与文件名都可能含用户名
    match fname.rfind('.') {
        Some(i) if fname.len() - i <= 6 => "***".to_string(),
        _ => "***".to_string(),
    }
}

#[cfg(test)]
mod tests;
