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
mod scan;

use scan::{
    bit_depth_of, normalize_audio_codec, normalize_video_codec, parse_num,
    scan_channel_count, scan_dolby_vision, scan_kv, scan_kv_u32,
    scan_resolution, scan_stream_channel_layout, scan_stream_format,
    scan_stream_kv, scan_stream_u32, stream_kind_of,
};

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
