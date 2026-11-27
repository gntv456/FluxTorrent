//! MediaInfo 文本扫描帮助函数族（2026-11-28 从 mod.rs 拆出守行数门禁）。
//! 全部纯字符串函数：吃 MediaInfo 导出的全文（lowercase），按流种类
//! （Video/Audio）与键名扫描数值/字符串；codec/位深规范化表也在此。
//! 仅被 mod.rs 的 parse/compare 消费——`pub(crate)` 足够。
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
pub(crate) fn stream_kind_of(l: &str) -> Option<&'static str> {
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
pub(crate) fn parse_num(v: &str) -> Option<u32> {
    let digits: String = v.chars().filter(|c| c.is_ascii_digit()).collect();
    if digits.is_empty() {
        return None;
    }
    digits.parse::<u32>().ok()
}

pub(crate) fn scan_stream_u32(low: &str, kind: &str, key: &str) -> Option<u32> {
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
pub(crate) fn scan_stream_channel_layout(low: &str) -> Option<u32> {
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
pub(crate) fn scan_stream_kv(
    low: &str,
    kind: &str,
    key: &str,
) -> Option<String> {
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
pub(crate) fn scan_resolution(low: &str) -> Option<(u32, u32)> {
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
pub(crate) fn scan_kv_u32(low: &str, key: &str) -> Option<u32> {
    scan_kv(low, key)?.trim().parse().ok()
}

pub(crate) fn scan_kv(low: &str, key: &str) -> Option<String> {
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
pub(crate) fn scan_stream_format(low: &str, kind: &str) -> Option<String> {
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

pub(crate) fn scan_channel_count(low: &str) -> Option<u32> {
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
pub(crate) fn scan_dolby_vision(low: &str) -> Option<String> {
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
pub(crate) fn bit_depth_of(pf: &str) -> Option<u32> {
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

pub(crate) fn normalize_video_codec(f: &str) -> Option<String> {
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

pub(crate) fn normalize_audio_codec(f: &str) -> Option<String> {
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
