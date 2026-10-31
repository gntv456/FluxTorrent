/* FluxTorrent <-> auto_feed 适配核心（纯函数，无 DOM）。
 *
 * 被 build.py 注入 auto_feed.user.js；Node 单测可直接 require 本文件。
 * 所有导出名以 Flux 开头，避免与上游 3 万行脚本的同名变量冲突。
 */
var FluxAutoFeedCore = (function () {
  "use strict";

  var LINK_SEP = "#linkstr#";
  // 转种时不自动勾选的标签（含官方同族）：这些是站方/发布组的行政标记，
  // 不该随内容转载到目标站。
  var SKIP_TAGS = ["禁转", "官种", "官方", "首发"];

  function text(v) {
    return v === null || v === undefined ? "" : String(v);
  }

  // 归一化：小写、去掉标点/空格，保留中英文数字。
  function norm(v) {
    return text(v)
      .toLowerCase()
      .replace(/[^\w\u3400-\u9fff]+/g, "");
  }

  function isSkippedTag(name) {
    var n = norm(name);
    if (!n) return false;
    for (var i = 0; i < SKIP_TAGS.length; i++) {
      var s = norm(SKIP_TAGS[i]);
      if (s && n.indexOf(s) >= 0) return true;
    }
    return false;
  }

  function hasAny(hay, aliases) {
    var h = norm(hay);
    if (!h) return false;
    for (var i = 0; i < aliases.length; i++) {
      var n = norm(aliases[i]);
      if (n && h.indexOf(n) >= 0) return true;
    }
    return false;
  }

  function clamp(v, max) {
    var s = text(v);
    return s.length > max ? s.slice(0, max) : s;
  }

  function findRow(rows, candidates) {
    if (!rows || !rows.length || !candidates || !candidates.length) {
      return null;
    }
    var i;
    var j;
    for (j = 0; j < candidates.length; j++) {
      for (i = 0; i < rows.length; i++) {
        if (norm(rows[i].name) === norm(candidates[j])) return rows[i];
      }
    }
    for (j = 0; j < candidates.length; j++) {
      for (i = 0; i < rows.length; i++) {
        if (hasAny(rows[i].name, [candidates[j]])) return rows[i];
      }
    }
    return null;
  }

  // ---------------------------------------------------------------- 链接
  function imdbId(v) {
    var m = text(v).match(/tt(\d{5,13})/i);
    return m ? "tt" + m[1] : "";
  }

  function imdbUrl(v) {
    var id = imdbId(v);
    return id ? "https://www.imdb.com/title/" + id + "/" : "";
  }

  function doubanUrl(descr) {
    var m = text(descr).match(
      /https?:\/\/(?:movie\.)?douban\.com\/subject\/(\d+)/i,
    );
    return m ? "https://movie.douban.com/subject/" + m[1] + "/" : "";
  }

  function doubanShort(v) {
    var m = text(v).match(/subject\/(\d+)/i);
    return m ? "https://movie.douban.com/subject/" + m[1] + "/" : "";
  }

  function firstImage(descr) {
    var m = text(descr).match(/\[img\](.*?)\[\/img\]/i);
    return m ? text(m[1]).trim() : "";
  }

  // 简介里的 MediaInfo（转种时最常见的载体：上传者把 MI 贴进 [quote]/[code]）。
  // 只认「General + MI 专有字段」或蓝光 DISC INFO，避免把剧情里的
  // “General / Format”误当 MI。
  function stripMarkup(s) {
    return text(s)
      .replace(/<br\s*\/?>/gi, "\n")
      .replace(/<\/p>/gi, "\n")
      .replace(/<[^>]+>/g, "")
      .replace(/\[\/?(?:quote|code|spoiler|hide|b|i|u|s|color|size|font|url|img|center|left|right|table|tr|td|list|\*)[^\]]*\]/gi, "")
      .replace(/&nbsp;/gi, " ")
      .replace(/&amp;/gi, "&")
      .replace(/&lt;/gi, "<")
      .replace(/&gt;/gi, ">");
  }

  function looksLikeMediaInfo(s) {
    var t = text(s);
    if (!t) return false;
    if (/DISC INFO:|Disc Label|\.MPLS/i.test(t)) return true;
    var general = /(^|\n)\s*General\b/i.test(t);
    if (!general) return false;
    var core = /(Complete name|Unique ID|Writing application|Writing library)/i
      .test(t);
    var format = /(^|\n)\s*Format\s*[:：]\s*[A-Za-z0-9.+\-_/ ]{2,40}/i
      .test(t);
    var stream = /(^|\n)\s*(Video|Audio|Text|Menu)\s*\n[\s\S]{0,500}?(Format|Codec ID|Bit rate|Duration)/i
      .test(t);
    return core || (format && stream);
  }

  /* 从整段正文里切出 MI：只从 General/DISC INFO 开始，到截图/剧情段前止。
   * 简介里 MI 常夹在正文中间，不切会把整段剧情灌进目标站 MI 框。 */
  function slicePlainMediaInfo(s) {
    var t = text(s);
    var m = t.match(/(^|\n)\s*(General|DISC INFO:|Disc Label)/i);
    if (!m) return "";
    var start = m.index + (m[1] ? m[1].length : 0);
    var tail = t.slice(start);
    var cut = tail.search(
      /\n\s*(?:\[img\]|\[url=|\[quote|<img|Screenshot|Screenshots|截图|截圖|◎|剧情简介|簡介|Plot|Synopsis)/i,
    );
    if (cut >= 0) tail = tail.slice(0, cut);
    return tail.trim();
  }

  function extractMediaInfo(descr) {
    var src = text(descr);
    if (!src) return "";
    var blocks = [];
    var re = /\[(quote|code|spoiler|hide)(?:=[^\]]*)?\]([\s\S]*?)\[\/\1\]/gi;
    var m;
    while ((m = re.exec(src))) blocks.push(m[2]);
    blocks.push(src);
    for (var i = 0; i < blocks.length; i++) {
      var cand = slicePlainMediaInfo(stripMarkup(blocks[i]));
      if (looksLikeMediaInfo(cand)) return cand;
    }
    return "";
  }

  // -------------------------------------------------------------- 标签
  function tagNamesFromAgg(agg) {
    var t = (agg && agg.tags) || {};
    var dict = t.dict || [];
    var mine = (t.mine || []).map(Number);
    var names = [];
    for (var i = 0; i < dict.length; i++) {
      var row = dict[i];
      var id = Array.isArray(row) ? row[0] : row && row.id;
      var name = Array.isArray(row) ? row[1] : row && row.name;
      if (mine.indexOf(Number(id)) >= 0) {
        var n = text(name).trim();
        if (n && !isSkippedTag(n) && names.indexOf(n) < 0) names.push(n);
      }
    }
    return names;
  }

  /* 把源站/上游载荷里的标签统一成可匹配的名字。
   * NP 的 labels 可能是数字位掩码（1=国语/10=粤语/100=中字）或字符串，
   * Unit3D/音乐站可能是逗号串；另外从标题/简介/MI 里补常见标签。 */
  function desiredTags(raw) {
    raw = raw || {};
    var out = [];
    function add(n) {
      n = text(n).trim();
      if (!n || /^\[object /i.test(n) || isSkippedTag(n) ||
        out.indexOf(n) >= 0) return;
      out.push(n);
    }
    var explicit = raw.tags;
    if (Array.isArray(explicit)) {
      explicit.forEach(add);
    } else if (typeof explicit === "string" && explicit) {
      explicit.split(/[,，\s]+/).forEach(add);
    }
    var lab = raw.labels;
    if (Array.isArray(lab)) {
      lab.forEach(add);
    } else if (lab && typeof lab === "object") {
      if (lab.zz) add("中字");
      if (lab.gy) add("国语");
      if (lab.yy) add("粤语");
      if (lab.yz) add("英字");
      if (lab.diy) add("DIY");
      if (lab.complete) add("完结");
      if (lab.hdr10 || lab.hdr10plus || lab.hdr) add("HDR");
      if (lab.db) add("杜比视界");
    } else if (typeof lab === "number" || /^\d+$/.test(text(lab))) {
      var bits = Number(lab);
      if (bits % 2) add("国语");
      if (bits > 9 && bits < 100) add("粤语");
      if (bits > 99) add("中字");
    } else if (typeof lab === "string" && lab) {
      lab.split(/[,，\s]+/).forEach(add);
    }
    var all = [raw.name, raw.small_descr, raw.descr].join("\n");
    if (/中字|中文字幕|简繁中字|簡繁中字|CHS|CHT/i.test(all)) add("中字");
    if (/国语|國語|国配|國配|普通话|普通話|mandarin/i.test(all)) {
      add("国语");
    }
    if (/粤语|粵語|广东话|廣東話|cantonese/i.test(all)) add("粤语");
    if (/英字|英文字幕|英语字幕|英語字幕/i.test(all)) add("英字");
    if (/DIY/i.test(all)) add("DIY");
    if (/独家|獨家/i.test(all)) add("独家");
    if (/HDR/i.test(all)) add("HDR");
    if (/Dolby Vision|杜比视界/i.test(all)) add("杜比视界");
    if (/特效字幕/i.test(all)) add("特效字幕");
    var title = [raw.name, raw.small_descr].join("\n");
    if (/合集|全集|\bPack\b/i.test(all) || /\bComplete\b/i.test(title)) {
      add("合集");
    }
    if (/完结|完結|全\d+集/i.test(all) || /\bComplete\b/i.test(title)) {
      add("完结");
    }
    return out;
  }

  function releaseGroup(name) {
    var m = text(name).match(/-([A-Za-z0-9@._]+)\s*$/);
    return m ? m[1] : "";
  }

  // ------------------------------------------------------------ 分类映射
  var UPSTREAM_TYPES = {
    "电影": ["电影", "movie", "film", "影片"],
    "剧集": ["电视剧", "剧集", "series", "tvseries"],
    "短剧": ["短剧", "电视剧", "series"],
    "动漫": ["动漫", "动画", "anime", "animation"],
    "综艺": ["综艺", "variety", "show"],
    "纪录": ["纪录片", "纪录", "documentary", "docu"],
    "音乐": ["音乐", "music"],
    "MV": ["音乐", "music", "mv"],
    "游戏": ["游戏", "game"],
    "软件": ["软件", "software", "app"],
    "学习": ["电子书", "书籍", "图书", "book", "ebook", "学习"],
    "体育": ["体育", "sport", "足球", "篮球"],
  };

  var CATEGORY_TYPES = [
    [/电影|movie|film|影片/i, "电影"],
    [/电视剧|剧集|series|tv.?show|剧/i, "剧集"],
    [/动漫|动画|anime|animation/i, "动漫"],
    [/综艺|variety|show/i, "综艺"],
    [/纪录|documentary|docu/i, "纪录"],
    [/音乐|music/i, "音乐"],
    [/游戏|game/i, "游戏"],
    [/软件|software|app/i, "软件"],
    [/电子书|书籍|图书|book|ebook|学习/i, "学习"],
    [/体育|sport|足球|篮球/i, "体育"],
  ];

  function categoryTypeOf(name) {
    var s = text(name);
    for (var i = 0; i < CATEGORY_TYPES.length; i++) {
      if (CATEGORY_TYPES[i][0].test(s)) return CATEGORY_TYPES[i][1];
    }
    return "";
  }

  function guessType(name) {
    var s = text(name);
    if (/S\d{1,3}(E\d{1,4})?|第.{1,4}季|EP?\d{1,4}/i.test(s)) {
      return "剧集";
    }
    if (/动漫|动画|anime/i.test(s)) return "动漫";
    if (/纪录片|documentary/i.test(s)) return "纪录";
    if (/综艺|variety/i.test(s)) return "综艺";
    if (/音乐|FLAC|MP3|OST/i.test(s)) return "音乐";
    if (/电影|movie|bluray|remux|web-dl/i.test(s)) return "电影";
    return "";
  }

  function categoryName(cats, id) {
    cats = cats || [];
    for (var i = 0; i < cats.length; i++) {
      if (Number(cats[i].id) === Number(id)) return text(cats[i].name);
    }
    return "";
  }

  function pickCategory(cats, raw) {
    cats = cats || [];
    var aliases = UPSTREAM_TYPES[text(raw && raw.type)] || [];
    if (!aliases.length && raw) aliases = UPSTREAM_TYPES[guessType(raw.name)] || [];
    if (!aliases.length) return null;
    for (var i = 0; i < cats.length; i++) {
      if (hasAny(cats[i].name, aliases)) return cats[i].id;
    }
    return null;
  }

  // ------------------------------------------------------ 媒介/分辨率/编码
  function mediumToUpstream(name, standardName) {
    var s = text(name);
    if (/remux/i.test(s)) return "Remux";
    if (/重编码|重編碼|重压|重壓|encode|bdrip/i.test(s)) return "Encode";
    if (/web-?rip/i.test(s)) return "WEBRip";
    if (/web-?dl|webdl|网络下载|網絡下載/i.test(s)) return "WEB-DL";
    if (/hdtv/i.test(s)) return "HDTV";
    if (/dvd/i.test(s)) return "DVD";
    var br = /蓝光|藍光|原盘|原盤|blu-?ray|bluray|bdmv|bd25|bd50|untouched|uhd/i;
    if (br.test(s)) {
      return /2160p|4k|uhd/i.test(text(standardName)) ? "UHD" : "Blu-ray";
    }
    return "";
  }

  function standardToUpstream(name) {
    var s = text(name);
    if (/4320p|8k/i.test(s)) return "8K";
    if (/2160p|4k|uhd/i.test(s)) return "4K";
    if (/1080i/i.test(s)) return "1080i";
    if (/1080p|fhd/i.test(s)) return "1080p";
    if (/720p/i.test(s)) return "720p";
    if (/480p|576p|sd/i.test(s)) return "SD";
    if (/1440p/i.test(s)) return "1440p";
    return "";
  }

  function codecToUpstream(name) {
    var s = text(name);
    if (/vvc|h\.?266/i.test(s)) return "H266";
    if (/h\.?265|hevc|x265/i.test(s)) return "H265";
    if (/h\.?264|avc|x264/i.test(s)) return "H264";
    if (/av1/i.test(s)) return "AV1";
    if (/vp9/i.test(s)) return "VP9";
    if (/vc-?1/i.test(s)) return "VC-1";
    if (/mpeg-?4/i.test(s)) return "MPEG-4";
    if (/mpeg-?2/i.test(s)) return "MPEG-2";
    if (/xvid/i.test(s)) return "XVID";
    if (/divx/i.test(s)) return "DIVX";
    return "";
  }

  function mediumCandidates(upstream) {
    switch (text(upstream)) {
      case "UHD":
        return ["原盘", "原盤", "蓝光", "藍光", "Blu-ray", "UHD", "2160p"];
      case "Blu-ray":
        return ["Blu-ray", "蓝光", "藍光", "原盘", "原盤"];
      case "Remux":
        return ["Remux", "原盘", "原盤", "蓝光", "藍光"];
      case "Encode":
        return ["重编码", "重編碼", "重压", "重壓", "Encode", "Blu-ray"];
      case "WEB-DL":
        return ["WEB-DL", "WEBRip"];
      case "WEBRip":
        return ["WEBRip", "WEB-DL"];
      case "HDTV":
        return ["HDTV"];
      case "DVD":
        return ["DVD"];
      default:
        return [];
    }
  }

  function standardCandidates(upstream) {
    switch (text(upstream)) {
      case "4K":
        return ["2160p", "4K", "UHD"];
      case "8K":
        return ["4320p", "8K"];
      case "1080p":
        return ["1080p", "FHD"];
      case "1080i":
        return ["1080i"];
      case "720p":
        return ["720p", "HD"];
      case "SD":
        return ["480p", "576p", "SD"];
      case "1440p":
      case "144Op":
        return ["1440p", "2K"];
      default:
        return [];
    }
  }

  function codecCandidates(upstream) {
    switch (text(upstream)) {
      case "H264":
        return ["H.264", "H264", "H.264/x264", "x264", "AVC", "AVC1"];
      case "X264":
        return ["H.264/x264", "x264", "H.264", "AVC"];
      case "H265":
        return ["H.265", "H265", "H.265/x265", "x265", "HEVC",
          "HEVC/x265"];
      case "X265":
        return ["H.265/x265", "x265", "H.265", "HEVC"];
      case "H266":
        return ["H.266", "H266", "VVC", "VVC/H.266"];
      case "AV1":
        return ["AV1"];
      case "VP9":
        return ["VP9"];
      case "VC-1":
        return ["VC-1", "VC1"];
      case "MPEG-2":
        return ["MPEG-2", "MPEG2"];
      case "MPEG-4":
        return ["MPEG-4", "MPEG4"];
      case "XVID":
        return ["XviD", "XVID"];
      case "DIVX":
        return ["DivX", "DIVX"];
      default:
        return [];
    }
  }

  // ------------------------------------------------------------ 源站解析
  function sectionName(entry) {
    if (!entry) return "";
    if (typeof entry === "string") return entry;
    if (entry.name) return text(entry.name);
    if (entry.values && entry.values.length) return text(entry.values[0]);
    return "";
  }

  // 维度 key 可能被站长改名（media→format、codec→video_codec…），
  // 先按 key，再按 label 兜底。
  function findSection(sec, keys, labelAliases) {
    var k;
    for (k in sec) {
      if (keys.indexOf(k) >= 0) return sectionName(sec[k]);
    }
    for (k in sec) {
      var entry = sec[k] || {};
      var label = text(entry.label);
      for (var i = 0; i < labelAliases.length; i++) {
        if (label.indexOf(labelAliases[i]) >= 0) return sectionName(entry);
      }
    }
    return "";
  }

  function legacyMedium(profile, mediumId) {
    var dict = (profile && profile.torrent_dicts &&
      profile.torrent_dicts.medium) || [];
    for (var i = 0; i < dict.length; i++) {
      if (Number(dict[i].id) === Number(mediumId)) return text(dict[i].name);
    }
    return "";
  }

  /* aggregate + site-profile -> auto_feed raw_info。
   * 不写 torrent_url（由适配层在源站带会话签发临时直链）。 */
  function buildRawInfo(agg, profile, originUrl) {
    var t = (agg && agg.torrent) || {};
    var d = (agg && agg.detail) || {};
    var cats = (profile && profile.categories) || [];
    var sec = d.sections || {};
    var catName = categoryName(cats, t.category_id);
    var mediaName = findSection(sec,
      ["media", "medium", "format"],
      ["媒介", "媒体", "媒體", "介质", "介質", "片源"]) ||
      legacyMedium(profile, t.medium_id);
    var stdName = findSection(sec,
      ["standard", "resolution", "quality"],
      ["分辨率", "解析度", "画质", "畫質", "清晰度"]);
    var codecName = findSection(sec,
      ["codec", "video_codec", "video_format"],
      ["编码", "編碼", "视频编码", "視頻編碼", "视频格式"]);
    var teamName = findSection(sec,
      ["team", "group", "release_group"],
      ["制作组", "製作組", "团队", "團隊", "发布组", "發布組"]);
    var descr0 = text(d.descr);
    var mediainfo = text(d.mediainfo).trim();
    if (!mediainfo) mediainfo = extractMediaInfo(descr0);
    var tags = tagNamesFromAgg(agg);
    var descr = descr0;
    var tagLine = tags.length ? "\n\n标签：" + tags.join(" ") : "";
    var already = tags.every(function (n) {
      return descr0.indexOf(n) >= 0;
    });
    if (tagLine && !already) {
      descr = (descr0.trim() + tagLine).trim();
    }
    var small = text(t.small_descr);
    var subtitleTags = tags.filter(function (n) {
      return /DIY|粤语|粵語/i.test(n);
    });
    if (subtitleTags.length &&
      !/DIY|粤语|粵語/i.test(small + " " + text(t.name))) {
      small = (small + " " + subtitleTags.join(" ")).trim();
    }
    // auto_feed 的 complete 只看 small_descr/name：给合集/完结补一个标记，
    // NP 目标才会勾「合集/完结」标签。
    if (tags.some(function (n) { return /合集|完结/.test(n); }) &&
      !/Complete|全集|全\d+集/i.test(small + " " + text(t.name))) {
      small = (small + " Complete").trim();
    }
    var imdb = imdbUrl(t.imdb_id);
    var douban = doubanUrl(descr0);
    var raw = {
      name: text(t.name),
      small_descr: small,
      descr: descr,
      poster: text(t.poster) || firstImage(descr0),
      url: imdb,
      dburl: douban,
      ptgen_url: imdb || douban,
      full_mediainfo: mediainfo,
      mediainfo: mediainfo,
      tags: tags.join(","),
      official: t.official ? "1" : "",
      type: categoryTypeOf(catName) || guessType(t.name),
      medium_sel: mediumToUpstream(mediaName, stdName),
      standard_sel: standardToUpstream(stdName),
      codec_sel: codecToUpstream(codecName),
      torrent_name: text(t.name),
      origin_site: "FluxTorrent",
      origin_url: text(originUrl),
    };
    var group = releaseGroup(t.name);
    if (teamName) raw.team_sel = teamName;
    else if (group) raw.team_sel = group;
    return raw;
  }

  // ------------------------------------------------------------ 目标计划
  /* raw_info + 站点档案 + section-dict -> 待填字段（纯数据，DOM 层执行）。
   * sections 的值：枚举存 dict_id 字符串，自由值存原文。 */
  function kindMatches(k, keys, aliases) {
    var key = text(k && k.kind);
    if (keys.indexOf(key) >= 0) return true;
    var label = text(k && k.label);
    for (var i = 0; i < aliases.length; i++) {
      if (label.indexOf(aliases[i]) >= 0) return true;
    }
    return false;
  }

  function planTargetFill(raw, profile, secDict) {
    raw = raw || {};
    profile = profile || {};
    secDict = secDict || {};
    var plan = {
      fields: {},
      category_id: null,
      sections: {},
    };
    plan.fields.name = clamp(raw.name, 200);
    plan.fields.small_descr = clamp(raw.small_descr, 120);
    plan.fields.descr = clamp(raw.descr, 59000);
    var mi = text(raw.full_mediainfo).trim() ||
      text(raw.mediainfo).trim() ||
      extractMediaInfo(raw.descr);
    plan.fields.mediainfo = clamp(mi, 59000);
    plan.fields.poster = text(raw.poster) || firstImage(raw.descr);
    plan.fields.imdb = imdbUrl(raw.url);
    plan.fields.douban = doubanShort(raw.dburl);
    plan.fields.ptgen = imdbUrl(raw.url) || doubanShort(raw.dburl) ||
      text(raw.ptgen_url);
    plan.tags = desiredTags(raw);

    var cat = pickCategory(profile.categories, raw);
    if (cat !== null) plan.category_id = cat;

    var kinds = secDict.kinds || [];
    for (var i = 0; i < kinds.length; i++) {
      var k = kinds[i];
      var rows = secDict[k.kind] || [];
      var cands = [];
      var isTeam = false;
      if (kindMatches(k, ["media", "medium", "format"],
        ["媒介", "媒体", "媒體", "介质", "介質", "片源", "格式", "media"])) {
        cands = mediumCandidates(raw.medium_sel);
      } else if (kindMatches(k, ["standard", "resolution", "quality"],
        ["分辨率", "解析度", "画质", "畫質", "清晰度", "standard"])) {
        cands = standardCandidates(raw.standard_sel);
      } else if (kindMatches(k, ["codec", "video_codec", "video_format"],
        ["编码", "編碼", "视频编码", "視頻編碼", "视频格式", "codec"])) {
        cands = codecCandidates(raw.codec_sel);
      } else if (kindMatches(k, ["team", "group", "release_group"],
        ["制作组", "製作組", "团队", "團隊", "发布组", "發布組",
          "team", "group"])) {
        isTeam = true;
        var team = text(raw.team_sel) || releaseGroup(raw.name);
        if (team) cands = [team];
      }
      var row = findRow(rows, cands);
      if (row) {
        plan.sections[k.kind] = String(row.id);
      } else if (isTeam && k.field_type === "text") {
        var freeTeam = text(raw.team_sel) || releaseGroup(raw.name);
        if (freeTeam) plan.sections[k.kind] = freeTeam;
      }
    }
    return plan;
  }

  // ------------------------------------------------------------ 载荷编解码
  /* 与上游 dictToString 同形：k1#linkstr#v1#linkstr#k2#linkstr#v2
   * 再 btoa(encodeURIComponent(...))。上游 dictToString 会先跑
   * fill_raw_info；适配层优先调用上游函数，这里只是测试/兜底实现。 */
  function encodePayload(raw) {
    var tmp = "";
    var key;
    for (key in raw) {
      tmp += key + LINK_SEP + text(raw[key]) + LINK_SEP;
    }
    tmp = tmp.slice(0, tmp.length - LINK_SEP.length);
    return btoa(encodeURIComponent(tmp));
  }

  function decodePayload(value) {
    var s = text(value).trim();
    var idx = s.indexOf("#separator#");
    if (idx >= 0) s = s.slice(idx + "#separator#".length).trim();
    s = s.replace(/^[?&=]+/, "");
    try {
      s = decodeURIComponent(s);
    } catch (e) {
      /* keep raw */
    }
    if (s.indexOf(LINK_SEP) < 0) {
      var b64 = s.match(/^[A-Za-z0-9+/=]+/);
      if (b64) s = b64[0];
      try {
        s = decodeURIComponent(atob(s));
      } catch (e2) {
        return {};
      }
    }
    var arr = s.split(LINK_SEP);
    var out = {};
    for (var i = 0; i + 1 < arr.length; i += 2) {
      out[arr[i]] = arr[i + 1];
    }
    return out;
  }

  return {
    LINK_SEP: LINK_SEP,
    text: text,
    norm: norm,
    clamp: clamp,
    findRow: findRow,
    imdbId: imdbId,
    imdbUrl: imdbUrl,
    doubanUrl: doubanUrl,
    doubanShort: doubanShort,
    firstImage: firstImage,
    extractMediaInfo: extractMediaInfo,
    looksLikeMediaInfo: looksLikeMediaInfo,
    tagNamesFromAgg: tagNamesFromAgg,
    desiredTags: desiredTags,
    isSkippedTag: isSkippedTag,
    SKIP_TAGS: SKIP_TAGS,
    releaseGroup: releaseGroup,
    categoryTypeOf: categoryTypeOf,
    guessType: guessType,
    pickCategory: pickCategory,
    mediumToUpstream: mediumToUpstream,
    standardToUpstream: standardToUpstream,
    codecToUpstream: codecToUpstream,
    mediumCandidates: mediumCandidates,
    standardCandidates: standardCandidates,
    codecCandidates: codecCandidates,
    buildRawInfo: buildRawInfo,
    planTargetFill: planTargetFill,
    encodePayload: encodePayload,
    decodePayload: decodePayload,
  };
})();

if (typeof module !== "undefined" && module.exports) {
  module.exports = FluxAutoFeedCore;
}
