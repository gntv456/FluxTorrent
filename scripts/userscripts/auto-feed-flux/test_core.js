/* core.js 单测：映射表、目标填充计划、载荷与上游编解码兼容性。
 *
 * 用法：node scripts/userscripts/auto-feed-flux/test_core.js
 * 上游文件存在时（_tmp_autofeed/auto_feed.user.js）会抽取其
 * dictToString/stringToDict 做双向对质；不存在则跳过。
 */
"use strict";

const assert = require("node:assert");
const fs = require("node:fs");
const path = require("node:path");
const core = require("./core.js");

const ROOT = path.resolve(__dirname, "../../..");
const UPSTREAM = path.join(ROOT, "_tmp_autofeed", "auto_feed.user.js");

const LIVE_CATEGORIES = [
  { id: 1, name: "电影" },
  { id: 2, name: "电视剧" },
  { id: 3, name: "音乐" },
  { id: 4, name: "游戏" },
  { id: 5, name: "软件" },
  { id: 6, name: "动漫" },
  { id: 7, name: "纪录片" },
  { id: 8, name: "电子书" },
  { id: 9, name: "体育" },
  { id: 10, name: "综艺" },
];

const LIVE_SECTIONS = {
  kinds: [
    { kind: "media", label: "媒介", field_type: "select" },
    { kind: "subtitle_group", label: "字幕组", field_type: "multiselect" },
    { kind: "codec", label: "编码", field_type: "select" },
    { kind: "standard", label: "分辨率", field_type: "select" },
    { kind: "team", label: "制作组", field_type: "select" },
  ],
  media: [
    { id: 44103, name: "原盘" },
    { id: 44105, name: "Blu-ray" },
    { id: 44100, name: "Remux" },
    { id: 44104, name: "重编码" },
    { id: 44102, name: "WEB-DL" },
    { id: 44099, name: "WEBRip" },
  ],
  standard: [
    { id: 44200, name: "2160p" },
    { id: 44201, name: "1080p" },
    { id: 44202, name: "720p" },
  ],
  codec: [
    { id: 44107, name: "H.264/x264" },
    { id: 44108, name: "H.265/x265" },
    { id: 44109, name: "AV1" },
  ],
  team: [
    { id: 44300, name: "OurBits" },
    { id: 44301, name: "FLUX" },
  ],
  subtitle_group: [],
};

function test(name, fn) {
  try {
    fn();
    console.log("ok   " + name);
  } catch (err) {
    console.error("FAIL " + name);
    throw err;
  }
}

test("分类语义", () => {
  assert.equal(core.categoryTypeOf("电影"), "电影");
  assert.equal(core.categoryTypeOf("电视剧"), "剧集");
  assert.equal(core.categoryTypeOf("动漫"), "动漫");
  assert.equal(core.categoryTypeOf("纪录片"), "纪录");
  assert.equal(core.categoryTypeOf("电子书"), "学习");
  assert.equal(core.categoryTypeOf("综艺"), "综艺");
});

test("pickCategory 按方言匹配", () => {
  assert.equal(core.pickCategory(LIVE_CATEGORIES, { type: "剧集" }), 2);
  assert.equal(core.pickCategory(LIVE_CATEGORIES, { type: "纪录" }), 7);
  assert.equal(core.pickCategory(LIVE_CATEGORIES, { type: "学习" }), 8);
  assert.equal(
    core.pickCategory(LIVE_CATEGORIES, { name: "Movie.2024.1080p" }),
    1,
  );
});

test("媒介/分辨率/编码反向映射", () => {
  assert.equal(core.mediumToUpstream("原盘", "2160p"), "UHD");
  assert.equal(core.mediumToUpstream("原盘", "1080p"), "Blu-ray");
  assert.equal(core.mediumToUpstream("重编码", ""), "Encode");
  assert.equal(core.mediumToUpstream("WEBRip", ""), "WEBRip");
  assert.equal(core.standardToUpstream("2160p"), "4K");
  assert.equal(core.standardToUpstream("1080i"), "1080i");
  assert.equal(core.codecToUpstream("H.265/x265"), "H265");
  assert.equal(core.codecToUpstream("MPEG-2"), "MPEG-2");
});

test("目标填充计划命中直播站字典", () => {
  const raw = {
    name: "Movie.2024.2160p.BluRay.x265-FLUX",
    small_descr: "4K 原盘",
    descr: "[img]https://img.example/a.jpg[/img]\n简介",
    type: "电影",
    medium_sel: "UHD",
    standard_sel: "4K",
    codec_sel: "H265",
    url: "https://www.imdb.com/title/tt15239678/",
  };
  const plan = core.planTargetFill(raw, { categories: LIVE_CATEGORIES },
    LIVE_SECTIONS);
  assert.equal(plan.category_id, 1);
  assert.equal(plan.fields.name, raw.name);
  assert.equal(plan.fields.poster, "https://img.example/a.jpg");
  assert.equal(plan.fields.imdb,
    "https://www.imdb.com/title/tt15239678/");
  assert.equal(plan.sections.media, "44103");
  assert.equal(plan.sections.standard, "44200");
  assert.equal(plan.sections.codec, "44108");
  assert.equal(plan.sections.team, "44301");
});

test("自定义维度 key 按标签兜底", () => {
  const sec = {
    kinds: [
      { kind: "format", label: "媒介", field_type: "select" },
      { kind: "resolution", label: "分辨率", field_type: "select" },
      { kind: "video_codec", label: "编码", field_type: "select" },
      { kind: "group", label: "制作组", field_type: "text" },
    ],
    format: [{ id: 1, name: "原盘" }],
    resolution: [{ id: 2, name: "2160p" }],
    video_codec: [{ id: 3, name: "H.265" }],
    group: [],
  };
  const plan = core.planTargetFill({
    name: "X.2026-FLUX",
    medium_sel: "UHD",
    standard_sel: "4K",
    codec_sel: "H265",
    team_sel: "FLUX",
  }, {}, sec);
  assert.equal(plan.sections.format, "1");
  assert.equal(plan.sections.resolution, "2");
  assert.equal(plan.sections.video_codec, "3");
  assert.equal(plan.sections.group, "FLUX");
});

test("MediaInfo 从简介的 quote/code/正文提取", () => {
  const mi = "General\nUnique ID : 1\nComplete name : test.mkv\n" +
    "Format : Matroska\n\nVideo\nFormat : HEVC\n\nAudio\nFormat : AAC";
  assert.equal(
    core.extractMediaInfo("[img]https://x/p.jpg[/img]\n简介\n[quote]" + mi +
      "[/quote]"),
    mi,
  );
  assert.equal(core.extractMediaInfo("[code]" + mi + "[/code]"), mi);
  assert.equal(core.extractMediaInfo("简介\n\n" + mi), mi);
  assert.equal(
    core.extractMediaInfo("DISC INFO:\nDisc Label: X\nSize: 1 GiB"),
    "DISC INFO:\nDisc Label: X\nSize: 1 GiB",
  );
  // 负控 1：剧情里的 General / Format 不是 MI
  assert.equal(
    core.extractMediaInfo("General 话题\n这是一段剧情\nFormat: 文字"),
    "",
  );
  // 负控 2：只有 General + Format、没有 MI 专有字段/流信息
  assert.equal(
    core.extractMediaInfo("General\nFormat : 文字\n没有流信息"),
    "",
  );
});

test("buildRawInfo / planTargetFill 的 MI 兜底与列优先", () => {
  const mi = "General\nComplete name : x.mkv\nFormat : Matroska\n\n" +
    "Video\nFormat : HEVC";
  const agg = {
    torrent: { name: "x", category_id: 1 },
    detail: {
      descr: "[quote]" + mi + "[/quote]",
      mediainfo: null,
      sections: {},
    },
  };
  const raw = core.buildRawInfo(agg, { categories: LIVE_CATEGORIES }, "");
  assert.equal(raw.full_mediainfo, mi);
  const plan = core.planTargetFill(
    { name: "x", descr: "[quote]" + mi + "[/quote]" },
    { categories: LIVE_CATEGORIES },
    { kinds: [] },
  );
  assert.equal(plan.fields.mediainfo, mi);
  const agg2 = {
    torrent: { name: "x", category_id: 1 },
    detail: {
      descr: "[quote]" + mi + "[/quote]",
      mediainfo: "General\nComplete name: from-column",
      sections: {},
    },
  };
  const raw2 = core.buildRawInfo(agg2, { categories: LIVE_CATEGORIES }, "");
  assert.equal(raw2.full_mediainfo, "General\nComplete name: from-column");
});

test("标签与 PT-Gen 计划", () => {
  const raw = {
    name: "X.2024.2160p-FLUX",
    small_descr: "国语 中字",
    descr: "简介",
    tags: "中字,DIY",
    url: "https://www.imdb.com/title/tt15239678/",
    official: "1",
  };
  const plan = core.planTargetFill(raw, {}, { kinds: [] });
  assert.equal(plan.fields.ptgen,
    "https://www.imdb.com/title/tt15239678/");
  for (const t of ["中字", "DIY", "国语"]) {
    assert.ok(plan.tags.includes(t), t);
  }
  assert.ok(!plan.tags.includes("官种"), JSON.stringify(plan.tags));
  assert.deepEqual(
    core.planTargetFill({ name: "x", labels: "100" }, {}, { kinds: [] })
      .tags,
    ["中字"],
  );
  assert.deepEqual(
    core.planTargetFill({ name: "x", labels: "10" }, {}, { kinds: [] })
      .tags,
    ["粤语"],
  );
  assert.deepEqual(
    core.planTargetFill({ name: "x", labels: "1" }, {}, { kinds: [] })
      .tags,
    ["国语"],
  );
  assert.ok(core.planTargetFill(
    { name: "x", labels: ["中字", "国语"] }, {}, { kinds: [] },
  ).tags.includes("中字"));
  const hdr = core.desiredTags({
    name: "x",
    descr: "HDR10 Dolby Vision 特效字幕",
  });
  assert.ok(hdr.includes("HDR"), JSON.stringify(hdr));
  assert.ok(hdr.includes("杜比视界"), JSON.stringify(hdr));
  assert.ok(hdr.includes("特效字幕"), JSON.stringify(hdr));
});

test("buildRawInfo 带标签 / PTGen / official", () => {
  const agg = {
    torrent: {
      name: "X.2024.1080p-FLUX",
      category_id: 1,
      imdb_id: "tt15239678",
      official: true,
    },
    detail: {
      descr: "简介",
      sections: {},
      mediainfo: "General\nComplete name: x\nFormat : Matroska\n\n" +
        "Video\nFormat : HEVC",
    },
    tags: {
      dict: [
        { id: 10, name: "中字", kind: "plain" },
        { id: 9, name: "国语", kind: "plain" },
      ],
      mine: [10, 9],
    },
  };
  const raw = core.buildRawInfo(agg, { categories: LIVE_CATEGORIES }, "");
  assert.equal(raw.tags, "中字,国语");
  assert.ok(raw.descr.includes("标签：中字 国语"));
  assert.equal(raw.ptgen_url,
    "https://www.imdb.com/title/tt15239678/");
  assert.equal(raw.official, "1");
  assert.equal(raw.full_mediainfo, agg.detail.mediainfo);
});

test("转种跳过禁转/官种/首发", () => {
  const tags = core.desiredTags({
    name: "X.2024",
    tags: "中字,禁转,官种,首发,官方,DIY",
    official: "1",
    descr: "禁转 首发 中字",
  });
  assert.deepEqual(tags.sort(), ["DIY", "中字"].sort());
  const agg = {
    torrent: { name: "X", category_id: 1 },
    detail: { descr: "简介", sections: {} },
    tags: {
      dict: [
        { id: 1, name: "中字", kind: "plain" },
        { id: 2, name: "禁转", kind: "plain" },
        { id: 3, name: "首发", kind: "plain" },
        { id: 4, name: "官种", kind: "official" },
      ],
      mine: [1, 2, 3, 4],
    },
  };
  const raw = core.buildRawInfo(agg, { categories: LIVE_CATEGORIES }, "");
  assert.equal(raw.tags, "中字");
  assert.ok(!raw.descr.includes("禁转"), raw.descr);
  assert.ok(!raw.descr.includes("首发"), raw.descr);
  assert.ok(raw.descr.includes("中字"), raw.descr);
});

test("MI 的 Complete name 不误判合集/完结", () => {
  const tags = core.desiredTags({
    name: "X.2024",
    descr: "[quote]General\nComplete name : x.mkv\nFormat : Matroska\n\n" +
      "Video\nFormat : HEVC[/quote]",
  });
  assert.ok(!tags.includes("合集"), JSON.stringify(tags));
  assert.ok(!tags.includes("完结"), JSON.stringify(tags));
});

test("自定义维度 key 与值别名（蓝光/HEVC/UHD）", () => {
  const agg = {
    torrent: { name: "X.2024-FLUX", category_id: 1 },
    detail: {
      descr: "简介",
      sections: {
        format: { name: "蓝光原盘", label: "媒体类型" },
        video_codec: { name: "HEVC", label: "视频编码" },
        resolution: { name: "UHD", label: "清晰度" },
        group: { name: "FLUX", label: "发布组" },
      },
    },
  };
  const raw = core.buildRawInfo(
    agg, { categories: LIVE_CATEGORIES }, "",
  );
  assert.equal(raw.medium_sel, "UHD");
  assert.equal(raw.codec_sel, "H265");
  assert.equal(raw.standard_sel, "4K");
  assert.equal(raw.team_sel, "FLUX");
  const sec = {
    kinds: [
      { kind: "format", label: "媒体类型", field_type: "select" },
      { kind: "video_codec", label: "视频编码", field_type: "select" },
      { kind: "resolution", label: "清晰度", field_type: "select" },
      { kind: "group", label: "发布组", field_type: "select" },
    ],
    format: [{ id: 1, name: "蓝光原盘" }],
    video_codec: [{ id: 3, name: "HEVC" }],
    resolution: [{ id: 4, name: "UHD" }],
    group: [{ id: 5, name: "FLUX" }],
  };
  const plan = core.planTargetFill({
    name: "X.2024-FLUX",
    medium_sel: "UHD",
    standard_sel: "4K",
    codec_sel: "H265",
    team_sel: "FLUX",
  }, { categories: LIVE_CATEGORIES }, sec);
  assert.equal(plan.sections.format, "1");
  assert.equal(plan.sections.video_codec, "3");
  assert.equal(plan.sections.resolution, "4");
  assert.equal(plan.sections.group, "5");
});

test("载荷编解码往返", () => {
  const raw = {
    name: "沙丘：复仇｜4K REMUX｜[简繁中字]",
    descr: "[img]https://a/b.png[/img]\n第二行 #x# 100%",
    type: "电影",
    medium_sel: "Remux",
  };
  const enc = core.encodePayload(raw);
  assert.ok(/^[A-Za-z0-9+/=]+$/.test(enc));
  const dec = core.decodePayload("#separator#" + enc);
  for (const k of Object.keys(raw)) assert.equal(dec[k], raw[k]);
});

function extractFn(src, name) {
  const start = src.indexOf("function " + name + "(");
  if (start < 0) return "";
  let i = src.indexOf("{", start);
  let depth = 0;
  for (; i < src.length; i++) {
    if (src[i] === "{") depth++;
    else if (src[i] === "}") {
      depth--;
      if (depth === 0) return src.slice(start, i + 1);
    }
  }
  return "";
}

test("与上游 dictToString/stringToDict 双向兼容", () => {
  if (!fs.existsSync(UPSTREAM)) {
    console.log("SKIP 上游文件不存在：" + UPSTREAM);
    return;
  }
  const src = fs.readFileSync(UPSTREAM, "utf8");
  const dictToString = extractFn(src, "dictToString");
  const stringToDict = extractFn(src, "stringToDict");
  const extractPayload = extractFn(src, "extractTransferPayload");
  const safeDecode = extractFn(src, "safeDecodeURIComponent");
  assert.ok(dictToString && stringToDict);
  const env = {
    separator: "#separator#",
    fill_raw_info: (x) => x,
    btoa,
    atob,
    encodeURIComponent,
    decodeURIComponent,
  };
  const factory = new Function(
    "env",
    "with (env) { " + dictToString + "\n" + stringToDict + "\n" +
      extractPayload + "\n" + safeDecode +
      "\nreturn { enc: dictToString, dec: stringToDict }; }",
  );
  const api = factory(env);
  const raw = {
    name: "沙丘：复仇｜2160p",
    descr: "[img]https://a/b.png[/img]\n正文 #x#",
    type: "电影",
  };
  const mine = core.encodePayload(raw);
  const theirs = api.enc(raw);
  assert.equal(mine, theirs, "encode 与上游不一致");
  const back = api.dec("#separator#" + mine);
  for (const k of Object.keys(raw)) assert.equal(back[k], raw[k]);
});

test("buildRawInfo 从 aggregate 取字段", () => {
  const agg = {
    torrent: {
      id: 1,
      name: "Movie.2024.2160p.BluRay.x265-FLUX",
      small_descr: "4K",
      category_id: 1,
      poster: null,
      imdb_id: "TT15239678",
      medium_id: 1,
    },
    detail: {
      descr: "[img]https://p/1.jpg[/img]\nhttps://movie.douban.com/subject/123/",
      mediainfo: "General\nComplete name: x",
      sections: {
        media: { name: "原盘" },
        standard: { name: "2160p" },
        codec: { name: "H.265/x265" },
      },
    },
  };
  const raw = core.buildRawInfo(agg, { categories: LIVE_CATEGORIES },
    "https://fluxtorrent.ptang.top/torrent/1");
  assert.equal(raw.name, agg.torrent.name);
  assert.equal(raw.type, "电影");
  assert.equal(raw.medium_sel, "UHD");
  assert.equal(raw.standard_sel, "4K");
  assert.equal(raw.codec_sel, "H265");
  assert.equal(raw.poster, "https://p/1.jpg");
  assert.equal(raw.url, "https://www.imdb.com/title/tt15239678/");
  assert.equal(raw.dburl, "https://movie.douban.com/subject/123/");
  assert.equal(raw.team_sel, "FLUX");
  assert.equal(raw.origin_site, "FluxTorrent");
});

console.log("all core tests passed");
