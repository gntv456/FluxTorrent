//! MediaInfo 解析与一致性比对的单测。
//!
//! 用例分三组：① 解析真实格式的 MediaInfo 输出；② 比对必须抓住
//! 典型标题党（假 4K、假 HDR、编码不符）；③ 脱敏不得泄露路径。

use super::*;

const MI_1080P_H264: &str = r#"
General
Complete name                            : /home/admin/secret/user_x264_show.S01E01.1080p.mkv
Format                                   : Matroska
File size                                : 2.0 GiB

Stream #0:0(eng): Video: AVC
ID                                       : 1
Format                                   : AVC
Format profile                           : High@L4.1
Width                                    : 1920
Height                                   : 1080
Pixel format                             : yuv420p
Color space                              : YUV
Transfer characteristics                : BT.709

Stream #0:1(eng): Audio: E-AC-3
ID                                       : 2
Format                                   : E-AC-3
Channel(s)                               : 6
Channel layout                           : 5.1
"#;

const MI_2160P_HEVC_HDR10: &str = r#"
General
Complete name                            : /mnt/rack3/DV.mkv
Format                                   : Matroska

Stream #0:0: Video: HEVC
ID                                       : 1
Format                                   : HEVC
Width                                    : 3840
Height                                   : 2160
Pixel format                             : yuv420p10le
Color space                              : YUV
Transfer characteristics                : SMPTE ST 2084 (PQ)
"#;

const MI_2160P_SDR: &str = r#"
General
Complete name                            : /mnt/fake4k.mkv

Stream #0:0: Video: HEVC
ID                                       : 1
Format                                   : HEVC
Width                                    : 3840
Height                                   : 2160
Pixel format                             : yuv420p10le
Color space                              : YUV
Transfer characteristics                : BT.709
"#;

// ── 解析 ──────────────────────────────────────────────────────────

#[test]
fn parses_1080p_h264_with_ddp_audio() {
    let s = parse(MI_1080P_H264);
    assert_eq!(s.resolution, Some((1920, 1080)));
    assert_eq!(s.video_codec.as_deref(), Some("h264"));
    assert!(!s.hdr, "BT.709 是 SDR");
    assert_eq!(s.audio_codec.as_deref(), Some("ddp"));
    // 5.1 → 6 声道
    assert_eq!(s.audio_channels, Some(6));
}

#[test]
fn parses_4k_hevc_hdr10_and_bit_depth() {
    let s = parse(MI_2160P_HEVC_HDR10);
    assert_eq!(s.resolution, Some((3840, 2160)));
    assert_eq!(s.video_codec.as_deref(), Some("hevc"));
    assert!(s.hdr);
    assert_eq!(s.hdr_format.as_deref(), Some("hdr10"));
    assert_eq!(s.bit_depth, Some(10), "yuv420p10le → 10 bit");
}

#[test]
fn parses_dash_x_resolution_form() {
    // "1920 x 1080" 形态（另一些工具的输出）
    let s = parse("Stream #0:0: Video: AVC\nWidth  : 1920 x 1080\n");
    assert_eq!(s.resolution, Some((1920, 1080)));
}

#[test]
fn malformed_text_yields_empty_not_panic() {
    // 解析器必须容错：用户常只贴片段，或贴完全无关的东西
    for t in ["", "不是 MediaInfo", "\n\n\n", "Video\nFormat :"] {
        let s = parse(t);
        assert!(s.resolution.is_none() || s.resolution.is_some());
    }
    // 明确断言：无法解析的字段留空，交给上层判「无法判定」
    assert!(parse("random text").resolution.is_none());
}

#[test]
fn video_format_is_not_confused_with_audio_format() {
    // 关键正确性：Format 字段在 Video 和 Audio 段各出现一次，
    // 必须取 Video 段的，不能被后面的 Audio 覆盖。
    let s = parse(MI_1080P_H264);
    assert_eq!(s.video_codec.as_deref(), Some("h264"));
    assert_ne!(s.video_codec.as_deref(), Some("ddp"));
}

// ── 比对：抓标题党 ────────────────────────────────────────────────

#[test]
fn fake_4k_title_is_caught() {
    let actual = parse(MI_1080P_H264);
    let d = Declared {
        resolution: Some("2160p".into()),
        ..Default::default()
    };
    let r = compare(&d, &actual);
    assert!(r.is_mismatch());
    let m = &r.mismatches[0];
    assert_eq!(m.field, "resolution");
    assert_eq!(m.declared, "3840x2160");
    assert_eq!(m.actual, "1920x1080");
}

#[test]
fn honest_4k_title_is_not_flagged() {
    let actual = parse(MI_2160P_HEVC_HDR10);
    let d = Declared {
        resolution: Some("2160p".into()),
        video_codec: Some("x265".into()),
        hdr: Some(true),
    };
    let r = compare(&d, &actual);
    assert!(!r.is_mismatch(), "如实标注不该被误判：{:?}", r.mismatches);
    assert!(r.unverifiable.is_empty());
}

#[test]
fn fake_hdr_title_is_caught() {
    let actual = parse(MI_2160P_SDR);
    let d = Declared {
        resolution: Some("2160p".into()),
        hdr: Some(true),
        video_codec: None,
    };
    let r = compare(&d, &actual);
    assert!(r.is_mismatch());
    assert_eq!(r.mismatches[0].field, "hdr");
    assert_eq!(r.mismatches[0].actual, "SDR");
}

#[test]
fn missing_hdr_is_not_flagged() {
    // 标题没写 HDR 但实际是 HDR：顶多是没写全，不是标题党
    let actual = parse(MI_2160P_HEVC_HDR10);
    let d = Declared {
        resolution: Some("2160p".into()),
        hdr: Some(false),
        video_codec: None,
    };
    assert!(!compare(&d, &actual).is_mismatch());
}

#[test]
fn wrong_codec_is_caught() {
    let actual = parse(MI_1080P_H264); // 实际 h264
    let d = Declared {
        resolution: Some("1080p".into()),
        video_codec: Some("x265".into()),
        hdr: None,
    };
    let r = compare(&d, &actual);
    assert!(r.is_mismatch());
    assert_eq!(r.mismatches[0].field, "video_codec");
    assert_eq!(r.mismatches[0].declared, "hevc");
    assert_eq!(r.mismatches[0].actual, "h264");
}

#[test]
fn unparseable_actual_becomes_unverifiable_not_mismatch() {
    // MediaInfo 缺信息时不得把「解析不出」当成「标题党」——
    // 误伤合法片源比放过几个标题党更伤用户信任。
    let actual = Summary::default(); // 全空
    let d = Declared {
        resolution: Some("2160p".into()),
        video_codec: Some("x265".into()),
        hdr: Some(true),
    };
    let r = compare(&d, &actual);
    assert!(!r.is_mismatch(), "解析不出不得判为不一致");
    assert!(r.unverifiable.contains(&"resolution".to_string()));
    assert!(r.unverifiable.contains(&"video_codec".to_string()));
    assert!(r.unverifiable.contains(&"hdr".to_string()));
}

#[test]
fn four_k_spelling_variants_all_accepted() {
    for s in ["4K", "2160p", "uhd", "UHD"] {
        assert_eq!(
            parse_declared_resolution(s),
            Some((3840, 2160)),
            "{s} 应解析为 3840x2160"
        );
    }
    for s in ["1080p", "FullHD", "FHD"] {
        assert_eq!(parse_declared_resolution(s), Some((1920, 1080)));
    }
}

#[test]
fn codec_spelling_variants_all_accepted() {
    for s in ["x264", "X.264", "H264", "AVC"] {
        assert_eq!(normalize_declared_codec(s).as_deref(), Some("h264"), "{s}");
    }
    for s in ["x265", "H.265", "HEVC"] {
        assert_eq!(normalize_declared_codec(s).as_deref(), Some("hevc"), "{s}");
    }
    // 认不出的写法不得瞎猜
    assert_eq!(normalize_declared_codec("乱七八糟"), None);
}

// ── 脱敏 ──────────────────────────────────────────────────────────

#[test]
fn sanitize_removes_local_paths() {
    let out = sanitize(MI_1080P_H264);
    assert!(!out.contains("/home/admin"), "泄露了目录：{out}");
    assert!(!out.contains("secret"), "泄露了目录名");
    assert!(!out.contains("user_x264_show"), "文件名也可能含用户名");
    // 但规格字段必须留着（脱敏不是把内容删光）
    assert!(out.contains("AVC"));
    assert!(out.contains("1920"));
}

#[test]
fn sanitize_masks_unc_hostname() {
    let text = r"Complete name : \\FILESRV01\share\movie.mkv";
    let out = sanitize(text);
    assert!(!out.contains("FILESRV01"), "UNC 主机名泄露：{out}");
    assert!(out.contains("***"));
}

#[test]
fn sanitize_keeps_non_path_lines_untouched() {
    let text = "Format : Matroska\nBit rate : 20 Mb/s";
    assert_eq!(sanitize(text), text);
}

#[test]
fn bit_depth_reads_the_right_digits() {
    // 回归：`yuv420p10le` 里既有色度采样 420 又有位深 10，
    // 把数字连起来读会得到 42010。
    for (pf, want) in [
        ("yuv420p10le", Some(10)),
        ("yuv420p", Some(8)),
        ("yuv444p12", Some(12)),
        ("yuv420p16le", Some(16)),
        // 认不出的格式不得硬报一个位深出来
        ("nv12", None),
        ("yuv420p99", None),
    ] {
        assert_eq!(bit_depth_of(pf), want, "像素格式 {pf}");
    }
}

#[test]
fn transfer_characteristics_is_read_in_plural_form() {
    // 回归：MediaInfo 的键是复数 "Transfer characteristics"。
    // 早前按单数查，HDR 全部漏判——于是「假 HDR」一个都抓不到，
    // 而这正是本模块的主要价值。
    let s = parse(MI_2160P_HEVC_HDR10);
    assert!(s.hdr);
    assert_eq!(s.hdr_format.as_deref(), Some("hdr10"));
}

#[test]
fn dolby_vision_is_recognized() {
    let text = "Stream #0:0: Video: HEVC\n\
                Pixel format : yuv420p10le\n\
                Dolby Vision ELG : 6 / BL RPU / EL RPU\n\
                Color space : YUV\n\
                Transfer characteristics : SMPTE ST 2084 (PQ)\n";
    let s = parse(text);
    assert!(s.hdr);
    assert_eq!(s.hdr_format.as_deref(), Some("dolby_vision"));
}

/// P-347：现代 MediaInfo 输出采用独立段落。
///
/// 原扫描器只认 `Stream #0:0: Video: ...` 这一套流头，遇到
/// 新版输出的 `Video` / `ID : Video 1` 格式时 `in_stream` 永远为 false，
/// 分辨率与编码被抽空——「假 4K」这类最典型标题党
/// 恰好从缏稍地漏过。同一份内容换成旧格式可正常解析，
/// 说明张本本身没错，错在扫描器不认第二套输出。
#[test]
fn modern_section_style_output_is_parsed() {
    let text = "General\n\
        Complete name                            : /home/admin/secret/user_x264_show\n\n\
        Video\n\
        ID                                       : Video 1\n\
        Format                                   : AVC\n\
        Width                                    : 1 920 pixels\n\
        Height                                   : 1 080 pixels\n\n\
        Audio\n\
        ID                                       : Audio 1\n\
        Format                                   : DTS\n\
        Channel layout                           : 5.1\n";
    let s = parse(text);
    assert_eq!(
        s.resolution,
        Some((1920, 1080)),
        "千分位空格 + 单位后缀必须能解出"
    );
    // AVC 经 normalize_video_codec 映射为 h264（与旧格式路径同一口径）。
    assert_eq!(s.video_codec.as_deref(), Some("h264"));
}

/// 同一份内容换成旧格式：两套输出必须结果一致。
#[test]
fn both_output_styles_agree() {
    let modern = "Video\n\
        ID                                       : Video 1\n\
        Format                                   : AVC\n\
        Width                                    : 1 920 pixels\n\
        Height                                   : 1 080 pixels\n";
    let legacy = "Stream #0:0: Video: AVC\n\
        Format                                   : AVC\n\
        Width                                    : 1 920 pixels\n\
        Height                                   : 1 080 pixels\n";
    assert_eq!(parse(modern).resolution, parse(legacy).resolution);
    assert_eq!(parse(modern).video_codec, parse(legacy).video_codec);
}

/// 真实格式下的「假 4K」必须被拦住——
/// 修前这个用例不会红（分辨率抽空 → 落入 unverifiable）。
#[test]
fn fake_4k_is_caught_in_modern_output() {
    let text = "Video\n\
        ID                                       : Video 1\n\
        Format                                   : AVC\n\
        Width                                    : 1 920 pixels\n\
        Height                                   : 1 080 pixels\n";
    let s = parse(text);
    let d = Declared {
        resolution: Some("2160p".into()),
        video_codec: None,
        hdr: None,
    };
    let r = compare(&d, &s);
    assert!(
        r.mismatches.iter().any(|m| m.field == "resolution"),
        "必须拦住假 4K，实际拿到的 mismatches={:?} unverifiable={:?}",
        r.mismatches,
        r.unverifiable
    );
}

/// 千分位数字的格式合理性：不得因为带空格就拿不到数字。
#[test]
fn thousand_separator_numbers_parse() {
    assert_eq!(parse_num("1 920 pixels"), Some(1920));
    assert_eq!(parse_num("1920 pixels"), Some(1920));
    assert_eq!(parse_num("23.976 FPS"), Some(23976));
    assert_eq!(parse_num(""), None);
    assert_eq!(parse_num("abc"), None);
}
