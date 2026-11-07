/* 产物补丁测试：直接抽取 dist 里的真实函数执行。
 *
 * 用法：node scripts/userscripts/auto-feed-flux/build.py
 *       node scripts/userscripts/auto-feed-flux/test_patches.js
 */
"use strict";

const assert = require("node:assert");
const fs = require("node:fs");
const path = require("node:path");
const core = require("./core.js");

const DIST = path.join(__dirname, "dist", "auto-feed-flux.user.js");

const tests = [];
// 排队执行，允许异步用例（真实 fillTorrentDirect 是 async）。
function test(name, fn) {
  tests.push({ name: name, fn: fn });
}

if (!fs.existsSync(DIST)) {
  console.error("缺少产物，先运行 build.py：" + DIST);
  process.exit(1);
}
const src = fs.readFileSync(DIST, "utf8");

function extractFn(text, name) {
  const start = text.indexOf("function " + name + "(");
  if (start < 0) return "";
  let i = text.indexOf("{", start);
  let depth = 0;
  for (; i < text.length; i++) {
    if (text[i] === "{") depth++;
    else if (text[i] === "}") {
      depth--;
      if (depth === 0) return text.slice(start, i + 1);
    }
  }
  return "";
}

test("头部：身份/版本/无自动更新", () => {
  assert.ok(src.includes("// @name         auto_feed (FluxTorrent)"));
  assert.ok(src.includes(
    "// @namespace    https://fluxtorrent.ptang.top/auto-feed"));
  assert.ok(src.includes("// @version      2.1.1.4.9"));
  assert.ok(!src.includes("@updateURL"));
  assert.ok(!/@require\s+https:\/\/greasyfork\.org\//.test(src));
});

test("头部：FluxTorrent 匹配范围", () => {
  for (const m of [
    "// @match        *://fluxtorrent.ptang.top/*",
    "// @match        *://ft.ptang.top/*",
    "// @match        *://mfwg.ptang.top/*",
    "// @match        *://www.hxpt.org/*",
    "// @match        *://www.hxpt.top/*",
    "// @match        *://localhost:3000/*",
    "// @match        *://*/torrent/*",
    "// @match        *://*/torrents*",
    "// @match        *://*/upload/*",
    "// @match        *://*/upload",
  ]) {
    assert.ok(src.includes(m), m);
  }
});

test("站点注册与三处接线", () => {
  assert.ok(src.includes(
    "'FluxTorrent': {'url': 'https://fluxtorrent.ptang.top/', 'enable': 1},"));
  assert.ok(src.includes(
    "if (fluxIsTorrentSite()) {\n" +
    "        FluxAutoFeedAdapter.main();"));
  assert.ok(src.includes("!fluxIsTorrentSite()"));
  assert.ok(src.includes("FluxAutoFeedCore"));
  assert.ok(src.includes("FluxAutoFeedAdapter"));
  assert.ok(src.includes("torrent_b64_key"));
  assert.ok(src.includes("flux_torrent_b64"));
});

test("真实 set_jump_href：FluxTorrent/好学 落 /upload 且载荷可解", () => {
  const code = extractFn(src, "set_jump_href");
  assert.ok(code, "未找到 set_jump_href");
  const anchors = {};
  const raw = {
    name: "Probe.2026.2160p",
    descr: "正文",
    type: "电影",
    url: "https://www.imdb.com/title/tt15239678/",
  };
  const env = {
    used_site_info: {
      FluxTorrent: { url: "https://fluxtorrent.ptang.top/", enable: 1 },
      好学: { url: "https://www.hxpt.org/", enable: 1 },
      HDCity: { url: "https://hdcity.city/", enable: 1 },
      TTG: { url: "https://totheglory.im/", enable: 1 },
      Disabled: { url: "https://disabled.invalid/", enable: 0 },
    },
    separator: "#separator#",
    dictToString: (r) => core.encodePayload(r),
    get_search_name: (n) => n,
    raw_info: raw,
    document: {
      getElementById: (id) => anchors[id] || null,
    },
  };
  for (const k of Object.keys(env.used_site_info)) {
    anchors[k] = { href: "" };
  }
  const fn = new Function(
    "env",
    "with (env) {" + code + "\nreturn set_jump_href;}",
  );
  fn(env)(raw, 1);
  assert.ok(
    anchors.FluxTorrent.href.startsWith(
      "https://fluxtorrent.ptang.top/upload#separator#"),
    anchors.FluxTorrent.href,
  );
  assert.ok(
    anchors["好学"].href.startsWith(
      "https://www.hxpt.org/upload#separator#"),
    anchors["好学"].href,
  );
  assert.ok(
    anchors.HDCity.href.startsWith(
      "https://hdcity.city/upload#separator#"),
    anchors.HDCity.href,
  );
  assert.ok(
    anchors.TTG.href.startsWith(
      "https://totheglory.im/upload.php#separator#"),
    anchors.TTG.href,
  );
  assert.equal(anchors.Disabled.href, "");
  const dec = core.decodePayload(anchors.FluxTorrent.href);
  assert.equal(dec.name, raw.name);
  assert.equal(dec.type, raw.type);
});

test("真实 set_jump_href 查重模式：FluxTorrent/好学 走 /torrents?search=", () => {
  const code = extractFn(src, "set_jump_href");
  assert.ok(code, "未找到 set_jump_href");
  const anchors = {};
  const raw = {
    name: "Probe.2026.2160p-WZW",
    descr: "正文",
    type: "电影",
    url: "https://www.imdb.com/title/tt15239678/",
  };
  const env = {
    used_site_info: {
      FluxTorrent: { url: "https://fluxtorrent.ptang.top/", enable: 1 },
      好学: { url: "https://www.hxpt.org/", enable: 1 },
      TTG: { url: "https://totheglory.im/", enable: 1 },
    },
    separator: "#separator#",
    dictToString: (r) => core.encodePayload(r),
    get_search_name: (n) => n,
    raw_info: raw,
    document: {
      getElementById: (id) => anchors[id] || null,
    },
  };
  // 上游给 String.prototype 挂了 format；本测试按需 stub 同名实现
  const fmtBackup = String.prototype.format;
  String.prototype.format = function (args) {
    return this.replace(/\{(\w+)\}/g, (m, k) =>
      Object.prototype.hasOwnProperty.call(args, k) ? args[k] : m);
  };
  for (const k of Object.keys(env.used_site_info)) {
    anchors[k] = { href: "" };
  }
  const fn = new Function(
    "env",
    "with (env) {" + code + "\nreturn set_jump_href;}",
  );
  try {
    fn(env)(raw, 0);
  } finally {
    String.prototype.format = fmtBackup;
  }
  assert.equal(
    anchors.FluxTorrent.href,
    "https://fluxtorrent.ptang.top/torrents?search=Probe.2026.2160p-WZW",
    anchors.FluxTorrent.href,
  );
  assert.equal(
    anchors["好学"].href,
    "https://www.hxpt.org/torrents?search=Probe.2026.2160p-WZW",
    anchors["好学"].href,
  );
  // 未特判的站仍走上游老行为
  assert.ok(anchors.TTG.href.includes("browse.php"), anchors.TTG.href);
});

test("真实 fill_torrent：FluxTorrent 写文件并派发 change", () => {
  const code = extractFn(src, "fill_torrent");
  assert.ok(code, "未找到 fill_torrent");
  let dispatched = 0;
  const input = {
    files: null,
    dispatchEvent: (ev) => {
      assert.ok(ev.type === "change" || ev.type === "input");
      dispatched++;
      return true;
    },
  };
  const form = { querySelector: () => input };
  const env = {
    $: () => {
      throw new Error("Flux 分支不应走 jQuery");
    },
    document: {
      querySelector: (sel) => {
        assert.ok(sel === "#upload-form" ||
          sel === 'input[type="file"]', sel);
        return sel === "#upload-form" ? form : input;
      },
    },
    Event,
    window: { File: function () {} },
  };
  const fn = new Function(
    "env",
    "with (env) {" + code + "\nreturn fill_torrent;}",
  );
  const files = { 0: { name: "x.torrent" }, length: 1 };
  fn(env)("FluxTorrent", { files }, "x.torrent");
  assert.equal(input.files, files);
  assert.equal(dispatched, 2, "change + input 都要派发");
  // 好学（同为 FluxTorrent 架构）走同一表单分支
  input.files = null;
  fn(env)("好学", { files }, "x.torrent");
  assert.equal(input.files, files);
  assert.equal(dispatched, 4);
});

test("真实 build_blob_from_torrent：目标 announce/private/source 改写", () => {
  const code = extractFn(src, "build_blob_from_torrent");
  assert.ok(code, "未找到 build_blob_from_torrent");
  // 最小合法私有种子（info 字典键按 bencode 排序，private 在最后）
  const pieces = "01234567890123456789";
  const info = "d6:lengthi1e4:name4:test12:piece lengthi16384e" +
    "6:pieces20:" + pieces + "7:privatei0ee";
  const announce = "http://localhost:3000/announce/abc";
  const rawTorrent = "d8:announce" + announce.length + ":" + announce +
    "4:info" + info + "e";
  let captured = null;
  const env = {
    alert: () => {},
    $: () => ({ length: 0 }),
    forward_site: "FluxTorrent",
    forward_announce: announce,
    Blob: function (parts, opts) {
      captured = { parts, type: opts && opts.type };
    },
    Uint8Array,
    parseInt,
    Math,
  };
  const fn = new Function(
    "env",
    "with (env) {" + code +
      "\nreturn build_blob_from_torrent;}",
  );
  fn(env)(rawTorrent, announce, "FluxTorrent", "application/x-bittorrent",
    (data) => {
      const bytes = captured.parts[0];
      let text = "";
      for (let i = 0; i < bytes.length; i++) {
        text += String.fromCharCode(bytes[i]);
      }
      captured.text = text;
      assert.equal(data.name, "");
    });
  assert.ok(captured, "callback 未执行");
  assert.ok(captured.text.startsWith(
    "d8:announce" + announce.length + ":" + announce), captured.text);
  assert.ok(captured.text.includes("8:encoding5:UTF-8"));
  assert.ok(captured.text.includes("7:privatei1e"), "private 未改成 1");
  assert.ok(captured.text.includes("6:source11:FLUXTORRENT"), "source 未写入");
  assert.ok(captured.text.endsWith("ee"), captured.text);
});

test("真实 build_blob_from_torrent：好学目标 source 按 UTF-8 字节计长", () => {
  const code = extractFn(src, "build_blob_from_torrent");
  assert.ok(code, "未找到 build_blob_from_torrent");
  const pieces = "01234567890123456789";
  const info = "d6:lengthi1e4:name4:test12:piece lengthi16384e" +
    "6:pieces20:" + pieces + "7:privatei0ee";
  const announce = "https://www.hxpt.org/announce/abc";
  const rawTorrent = "d8:announce" + announce.length + ":" + announce +
    "4:info" + info + "e";
  let captured = null;
  const env = {
    alert: () => {},
    $: () => ({ length: 0 }),
    Blob: function (parts) {
      captured = parts[0];
    },
    Uint8Array,
    parseInt,
    Math,
  };
  const fn = new Function(
    "env",
    "with (env) {" + code +
      "\nreturn build_blob_from_torrent;}",
  );
  fn(env)(rawTorrent, announce, "好学", "application/x-bittorrent",
    () => {});
  assert.ok(captured, "callback 未执行");
  let text = "";
  for (let i = 0; i < captured.length; i++) {
    text += String.fromCharCode(captured[i]);
  }
  // 若按 UTF-16 长度写会产出 6:source2:好学（截断/坏 bencode）；
  // 修正后统一 FLUXTORRENT（11 字节），与 FluxTorrent 目标一致。
  assert.ok(text.includes("6:source11:FLUXTORRENT"),
    "好学目标 source 标签错误：" + text.slice(-40));
});

test("真实 addTorrent b64 快路径：全局 raw_info 命中时种子文件直接填入表单", () => {
  const at = extractFn(src, "addTorrent");
  const ft = extractFn(src, "fill_torrent");
  assert.ok(at && ft, "未找到 addTorrent/fill_torrent");
  const fileInput = {
    files: null,
    dispatched: 0,
    dispatchEvent() { this.dispatched++; return true; },
  };
  class FakeBlob {
    constructor(parts) { this.parts = parts; this.type = "application/x-bittorrent"; }
  }
  class FakeFile extends FakeBlob {
    constructor(parts, name) { super(parts); this.name = name; }
  }
  class FakeDT {
    constructor() {
      this._l = [];
      const self = this;
      this.items = { add(f) { self._l.push(f); } };
    }
    get files() { return this._l; }
  }
  const gmStore = {
    flux_torrent_b64: JSON.stringify({
      key: "k1", b64: "eA==", at: Date.now(),
    }),
  };
  // 好学线上 section-dict 实词表回归：source(kind=source,label=来源)
  const deps = {
    GM_getValue: (k) => gmStore[k],
    console,
    alert: () => {},
    raw_info: { torrent_b64_key: "k1", torrent_name: "T.2026.1080p" },
    atob: () => "x",
    window: { File: FakeFile },
    DataTransfer: FakeDT,
    build_blob_from_torrent: (bin, ann, site, type, cb) =>
      cb({ data: new FakeBlob([new Uint8Array([1])]) }),
    fill_torrent: null,
  };
  const fakeForm = { querySelector: () => fileInput };
  const ftDeps = {
    $: () => [{ files: null, dispatchEvent: () => true }],
    document: {
      querySelector: (sel) =>
        sel.includes("#upload-form") ? fakeForm : fileInput,
    },
    Event: class { constructor(t) { this.type = t; } },
  };
  deps.fill_torrent = new Function(
    ...Object.keys(ftDeps), ft + "\nreturn fill_torrent;",
  )(...Object.values(ftDeps));
  const atFn = new Function(
    ...Object.keys(deps), at + "\nreturn addTorrent;",
  )(...Object.values(deps));
  atFn("https://expired.example/x", "T.2026.1080p", "好学",
    "https://www.hxpt.org/announce/xx");
  const names = fileInput.files.map((f) => f.name);
  assert.deepEqual(names, ["T.2026.1080p.torrent"], String(names));
  assert.equal(fileInput.dispatched, 2, "change + input 未派发");
  // 快路径命中时不应触发跨站 getBlob（此处无 GM_xmlhttpRequest 依赖即通过）
});

test("好学线上词表回放：source/standard/codec 全命中", () => {
  // 2026-11-07 实取 https://www.hxpt.org/api/v1/section-dict?category_id=401
  const dict = {
    media: [{ id: 1, name: "视频" }, { id: 9, name: "其他" }],
    source: [
      { id: 10, name: "WEB-DL" }, { id: 20, name: "WEBRip" },
      { id: 30, name: "BluRay" }, { id: 40, name: "HDTV" },
      { id: 50, name: "DVDRip" }, { id: 90, name: "其他" },
    ],
    standard: [
      { id: 2, name: "720p" }, { id: 3, name: "1080p" },
      { id: 4, name: "2160p/4K" }, { id: 5, name: "8K" },
      { id: 6, name: "其他" },
    ],
    codec: [
      { id: 1, name: "H.264" }, { id: 2, name: "H.265" },
      { id: 3, name: "AV1" }, { id: 4, name: "XviD" },
    ],
  };
  const kinds = [
    { kind: "media", label: "媒介" },
    { kind: "source", label: "来源" },
    { kind: "standard", label: "分辨率" },
    { kind: "codec", label: "编码" },
  ];
  const cases = [
    [{ medium_sel: "WEB-DL", standard_sel: "1080p", codec_sel: "H264" },
      { source: "10", standard: "3", codec: "1" }],
    [{ medium_sel: "Remux", standard_sel: "4K", codec_sel: "H265" },
      { source: "30", standard: "4", codec: "2" }],
    [{ medium_sel: "UHD", standard_sel: "4K", codec_sel: "H265" },
      { source: "30", standard: "4", codec: "2" }],
    [{ medium_sel: "Encode", standard_sel: "1080p", codec_sel: "H265" },
      { source: "30", standard: "3", codec: "2" }],
    [{ medium_sel: "HDTV", standard_sel: "720p", codec_sel: "H264" },
      { source: "40", standard: "2", codec: "1" }],
    [{ medium_sel: "WEBRip", standard_sel: "1080p", codec_sel: "H264" },
      { source: "20", standard: "3", codec: "1" }],
    [{ medium_sel: "DVD", codec_sel: "XVID" },
      { source: "50", codec: "4" }],
  ];
  for (const [rawIn, want] of cases) {
    const plan = core.planTargetFill(
      Object.assign({ name: "X" }, rawIn), { categories: [] },
      Object.assign({ kinds }, dict),
    );
    for (const kindKey of Object.keys(want)) {
      assert.equal(
        plan.sections[kindKey], want[kindKey],
        rawIn.medium_sel + "/" + kindKey + " 期望 " + want[kindKey] +
          " 实得 " + plan.sections[kindKey],
      );
    }
  }
});

test("真实 stringToDict：载荷往返保留 torrent_b64_key", () => {
  // 目标站 handleTarget 用 stringToDict(site_url) 解析；若载荷丢了这个
  // key，写全局 raw_info 也救不回 addTorrent 的 b64 快路径。
  const code = extractFn(src, "stringToDict");
  const safe = extractFn(src, "safeDecodeURIComponent");
  const extract = extractFn(src, "extractTransferPayload");
  assert.ok(code && safe && extract, "未找到 stringToDict 辅助函数");
  const env = {
    separator: "#separator#",
    URL,
    atob,
    decodeURIComponent,
    link_str: undefined,
  };
  const fn = new Function(
    "env",
    "with (env) {" + safe + "\n" + extract + "\n" + code +
      "\nreturn stringToDict;}",
  );
  const raw = {
    name: "T.2026.2160p",
    descr: "正文",
    torrent_b64_key: "k-20261107-abc",
    content_kind: "video",
  };
  const payload = core.encodePayload(raw);
  const decoded = fn(env)(payload);
  assert.equal(decoded.name, raw.name);
  assert.equal(decoded.torrent_b64_key, raw.torrent_b64_key,
    "载荷往返丢了 torrent_b64_key");
  assert.equal(decoded.content_kind, "video",
    "载荷往返丢了 content_kind");
  // 带 #separator# 的整条跳转链接同样要能解出 key
  const decoded2 = fn(env)(
    "https://www.hxpt.org/upload#separator#" + payload);
  assert.equal(decoded2.torrent_b64_key, raw.torrent_b64_key);
});

test("真实 handleTarget：解析载荷后同步写入全局 raw_info", () => {
  // 上游 var raw_info 在脚本顶层；adapter IIFE 是 strict，作用域链
  // 不可达时赋值会抛 ReferenceError。此用例在 vm 里复现真实链路。
  const vm = require("node:vm");
  // site 判定辅助函数（fluxSiteMode 等）定义在 IIFE 之外，须一并注入
  const start = src.indexOf("var FLUX_SITE_HOSTS = ");
  assert.ok(start > 0, "未找到适配层起始");
  const tail = "\n})();";
  const end = src.indexOf(tail, start);
  assert.ok(end > start, "未找到 adapter IIFE 结尾");
  const iife = src.slice(start, end + tail.length);
  const payload = {
    name: "T.2026.2160p",
    type: "电影",
    torrent_b64_key: "k-20261107-abc",
  };
  let nowCalls = 0;
  const node = new Proxy({}, {
    get: (t, k) => (k === "style" ? {} : () => node),
  });
  const ctx = {
    FluxAutoFeedCore: core,
    raw_info: { name: "上游初始值" },
    stringToDict: () => payload,
    site_url: "https://www.hxpt.org/upload#separator#x",
    location: { href: "https://www.hxpt.org/upload",
      pathname: "/upload", hostname: "www.hxpt.org" },
    window: {},
    document: {
      querySelector: () => null,
      createElement: () => node,
      body: node,
      addEventListener: () => {},
    },
    Date: { now: () => (nowCalls++ ? 1e12 : 0) },
    setTimeout: () => 0,
    clearTimeout: () => {},
    GM_getValue: () => undefined,
    GM_setValue: () => {},
    console,
    Promise,
    URL,
    atob,
    btoa,
    encodeURIComponent,
    decodeURIComponent,
  };
  vm.createContext(ctx);
  vm.runInContext(
    iife + "\n;globalThis.__fluxAdapter = FluxAutoFeedAdapter;", ctx);
  const adapter = ctx.__fluxAdapter;
  assert.ok(adapter && adapter._test && adapter._test.handleTarget,
    "adapter._test.handleTarget 不可用");
  // raw_info = raw 是第一个 await 之前的同步语句，调用后立即可断言；
  // 之后的填充流程不在本用例范围，吞掉 rejection 避免未处理异常。
  adapter._test.handleTarget().catch(() => {});
  assert.equal(ctx.raw_info, payload, "全局 raw_info 未被载荷覆盖");
  assert.equal(ctx.raw_info.torrent_b64_key, "k-20261107-abc");
});

test("真实 set_jump_href 查重：种名特殊字符已编码", () => {
  const code = extractFn(src, "set_jump_href");
  assert.ok(code, "未找到 set_jump_href");
  const weird = "A B&C#D+E.2026.1080p";
  const cases = [
    { url: "https://www.imdb.com/title/tt15239678/" }, // IMDb 循环
    { url: "" },                                       // 普通名循环
  ];
  for (const extra of cases) {
    const anchors = {};
    const env = {
      used_site_info: {
        FluxTorrent: { url: "https://fluxtorrent.ptang.top/", enable: 1 },
        好学: { url: "https://www.hxpt.org/", enable: 1 },
      },
      separator: "#separator#",
      dictToString: () => "eA==",
      get_search_name: (n) => n,
      raw_info: Object.assign({ name: weird, descr: "正文",
        type: "电影" }, extra),
      document: { getElementById: (id) => anchors[id] || null },
      encodeURIComponent,
    };
    const fmtBackup = String.prototype.format;
    String.prototype.format = function (args) {
      return this.replace(/\{(\w+)\}/g, (m, k) =>
        Object.prototype.hasOwnProperty.call(args, k) ? args[k] : m);
    };
    for (const k of Object.keys(env.used_site_info)) {
      anchors[k] = { href: "" };
    }
    const fn = new Function(
      "env", "with (env) {" + code + "\nreturn set_jump_href;}");
    try {
      fn(env)(env.raw_info, 0);
    } finally {
      String.prototype.format = fmtBackup;
    }
    const want = "torrents?search=" + encodeURIComponent(weird);
    for (const k of Object.keys(env.used_site_info)) {
      assert.ok(anchors[k].href.includes(want),
        k + " 查重链接未编码：" + anchors[k].href);
      assert.ok(!anchors[k].href.includes("search=A B"),
        k + " 查重链接含裸空格：" + anchors[k].href);
    }
  }
});

test("产物 core：好学词表影视类 media 选中视频", () => {
  const start = src.indexOf("var FluxAutoFeedCore = (function () {");
  const end = src.indexOf("\n})();", start);
  assert.ok(start > 0 && end > start, "未找到产物里的 core IIFE");
  const built = new Function(
    src.slice(start, end + "\n})();".length) +
      "\nreturn FluxAutoFeedCore;")();
  const dict = {
    kinds: [
      { kind: "media", label: "媒介" },
      { kind: "source", label: "来源" },
      { kind: "standard", label: "分辨率" },
      { kind: "codec", label: "编码" },
    ],
    media: [
      { id: 1, name: "视频" }, { id: 2, name: "音频" },
      { id: 3, name: "文档" }, { id: 9, name: "其他" },
    ],
    source: [{ id: 10, name: "WEB-DL" }, { id: 30, name: "BluRay" }],
    standard: [{ id: 3, name: "1080p" }],
    codec: [{ id: 1, name: "H.264" }],
  };
  for (const t of ["电影", "剧集", "综艺", "动漫", "纪录", "体育"]) {
    const plan = built.planTargetFill({
      name: "X.2026", type: t, medium_sel: "WEB-DL",
      standard_sel: "1080p", codec_sel: "H264",
    }, { categories: [] }, dict);
    assert.equal(plan.sections.media, "1", t + " 应归视频");
    assert.equal(plan.sections.source, "10", t + " 来源维度仍要命中");
    assert.equal(plan.sections.standard, "3");
    assert.equal(plan.sections.codec, "1");
  }
  // 种子文件格式信号优先：音乐分类但种子里装的是演唱会视频
  const concert = built.planTargetFill({
    name: "X", type: "音乐", content_kind: "video",
  }, {}, dict);
  assert.equal(concert.sections.media, "1");
});

test("产物 adapter：种子准备时把文件格式写进载荷", () => {
  const anchor =
    "raw.content_kind = core.contentKindFromFiles(bin);";
  assert.ok(src.includes(anchor), "prepareTorrentB64 未写入 content_kind");
  const start = src.indexOf("async function prepareTorrentB64()");
  const at = src.indexOf(anchor, start);
  const key = src.indexOf("raw.torrent_b64_key = key;", start);
  assert.ok(start > 0 && at > start && key > at,
    "content_kind 必须在写 b64 key 前落进 raw（ensureLinks 会重建载荷）");
});

test("真实 fillTorrentDirect：缓存命中直填文件框并触发 onChange",
  async () => {
    const vm = require("node:vm");
    const start = src.indexOf("var FLUX_SITE_HOSTS = ");
    const end = src.indexOf("\n})();", start);
    const iife = src.slice(start, end + "\n})();".length);
    const onChangeCalls = [];
    const input = {
      files: null,
      dispatched: [],
      dispatchEvent(e) { this.dispatched.push(e.type); },
      __reactProps$test: {
        onChange(e) { onChangeCalls.push(e.target === input); },
      },
    };
    const form = {
      querySelector(sel) {
        return sel.indexOf('accept*=".torrent"') >= 0 ? input : null;
      },
    };
    class FakeBlob {
      constructor(parts) {
        this.parts = parts;
        this.type = "application/x-bittorrent";
      }
    }
    class FakeFile extends FakeBlob {
      constructor(parts, name) { super(parts); this.name = name; }
    }
    class FakeDT {
      constructor() {
        this._l = [];
        const self = this;
        this.items = { add(f) { self._l.push(f); } };
      }
      get files() { return this._l; }
    }
    const now = Date.now();
    let gm = JSON.stringify({ key: "k1", b64: "ZDE6YQ==", at: now });
    const ctx = {
      FluxAutoFeedCore: core,
      raw_info: {},
      GM_getValue: (k) => k === "flux_torrent_b64" ? gm : undefined,
      GM_setValue: () => {},
      GM_xmlhttpRequest: () => {},
      build_blob_from_torrent: (bin, ann, site, type, cb) =>
        cb({ data: new FakeBlob([new Uint8Array([100])]) }),
      atob: (s) => Buffer.from(s, "base64").toString("binary"),
      window: { File: FakeFile },
      DataTransfer: FakeDT,
      Event: class { constructor(t) { this.type = t; } },
      Date, Math, Promise, console, setTimeout, clearTimeout,
      location: {
        href: "https://www.hxpt.org/upload#separator#x",
        pathname: "/upload", hostname: "www.hxpt.org",
        origin: "https://www.hxpt.org",
      },
      document: {
        querySelector: () => null,
        createElement: () => ({ style: {} }),
        body: { appendChild: () => {} },
        addEventListener: () => {},
      },
      encodeURIComponent, decodeURIComponent, URL, btoa,
    };
    vm.createContext(ctx);
    vm.runInContext(
      iife + "\n;globalThis.__fluxA = FluxAutoFeedAdapter;", ctx);
    const fill = ctx.__fluxA._test.fillTorrentDirect;
    const raw = { name: "T.2026", torrent_b64_key: "k1" };
    const res = await fill(raw, form);
    assert.equal(res.ok, true, res.reason);
    assert.equal(input.files.length, 1);
    assert.equal(input.files[0].name, "T.2026.torrent");
    assert.ok(input.dispatched.includes("change"));
    assert.equal(onChangeCalls.length, 1, "React onChange 未触发");
    // 每一步失败都带原因，不再静默无反应
    assert.match((await fill({ name: "X" }, form)).reason,
      /缓存键/);
    gm = JSON.stringify({ key: "other", b64: "eA==", at: now });
    assert.match((await fill(raw, form)).reason, /缓存键不匹配/);
    gm = JSON.stringify({ key: "k1", b64: "eA==",
      at: now - 31 * 60 * 1000 });
    assert.match((await fill(raw, form)).reason, /30 分钟/);
    gm = undefined;
    assert.match((await fill(raw, form)).reason, /无种子缓存/);
    // 任何异常都不得冒泡到 handleTarget 把整段填充标成失败
    const boom = await fill(null, form);
    assert.equal(boom.ok, false);
    assert.match(boom.reason, /直填异常/);
    // 第三方 PT 站：载荷无缓存键，改走跨站拉源站直链
    const third = { name: "T.3rd", torrent_url: "https://src/x" };
    const bencode = "d8:announce9:https://x4:infodee";
    const encBytes = new Uint8Array(bencode.length);
    for (let i = 0; i < bencode.length; i++) {
      encBytes[i] = bencode.charCodeAt(i);
    }
    ctx.GM_xmlhttpRequest = (opt) =>
      opt.onload({ status: 200, response: encBytes.buffer });
    input.files = null;
    const r3 = await fill(third, form);
    assert.equal(r3.ok, true, r3.reason);
    assert.equal(input.files[0].name, "T.3rd.torrent");
    // 失败按类型分开报，不再静默
    ctx.GM_xmlhttpRequest = (opt) => opt.onload({ status: 403 });
    assert.match((await fill(third, form)).reason, /HTTP 403/);
    ctx.GM_xmlhttpRequest = (opt) => {
      const html = new Uint8Array([60, 104, 116, 109, 108, 62]);
      opt.onload({ status: 200, response: html.buffer });
    };
    assert.match((await fill(third, form)).reason, /不是 \.torrent/);
  });

test("产物 fill_torrent：Flux 分支多选择器 + React onChange 兜底", () => {
  const code = extractFn(src, "fill_torrent");
  assert.ok(code, "未找到 fill_torrent");
  assert.ok(code.includes("var fluxForm"), "缺少 form 内查找");
  assert.ok(code.includes('input[name="file"][type="file"]'),
    "缺少 name=file 兜底选择器");
  assert.ok(code.includes("__reactProps$"), "缺少 React onChange 兜底");
  assert.ok(code.includes("torrent input not found"), "缺少失败日志");
  const input = {
    files: null,
    dispatched: [],
    dispatchEvent(e) { this.dispatched.push(e.type); },
    __reactProps$x: { onChange() { this.called = true; } },
  };
  const form = { querySelector: () => input };
  const env = {
    document: { querySelector: () => form },
    $: () => { throw new Error("不应走 jQuery 兜底"); },
    Event: class { constructor(t) { this.type = t; } },
    console,
  };
  const fn = new Function(
    "env", "with (env) {" + code + "\nreturn fill_torrent;}");
  fn(env)("好学", { files: ["X.torrent"] }, "X");
  assert.deepEqual(input.files, ["X.torrent"]);
  assert.ok(input.dispatched.includes("change"));
  assert.ok(input.__reactProps$x.called, "onChange 未调用");
});

test("产物含版本指纹与诊断口（F12 可确认已装新版）", () => {
  assert.ok(src.includes("// @version      2.1.1.4.9"));
  assert.ok(src.includes('ADAPTER_BUILD = "2026-11-07e"'),
    "缺少新版本指纹");
  assert.ok(src.includes("function diag()"), "缺少诊断口");
  assert.ok(src.includes("diag: diag"), "诊断口未导出到 _test");
  assert.ok(src.includes("showBuildBadge"), "缺少可见版本徽标");
  assert.ok(src.includes("fillTorrentDirectInner"), "缺少异常兜底包装");
  assert.ok(src.includes("种子重建超时（5s）"), "缺少 callback 超时兜底");
});

test("真实 diag：一次拿到载荷/缓存/DOM 排障状态", () => {
  const vm = require("node:vm");
  const start = src.indexOf("var FLUX_SITE_HOSTS = ");
  const end = src.indexOf("\n})();", start);
  const iife = src.slice(start, end + "\n})();".length);
  const input = { files: [{ name: "x.torrent" }] };
  const form = { querySelector: () => input };
  const ctx = {
    FluxAutoFeedCore: core,
    raw_info: {},
    GM_getValue: () => JSON.stringify({ key: "kk", at: Date.now() }),
    location: {
      href: "https://www.hxpt.org/upload#separator#x",
      pathname: "/upload", hostname: "www.hxpt.org",
      origin: "https://www.hxpt.org",
    },
    document: {
      querySelector: (sel) => sel === "#upload-form" ? form : null,
      cookie: "", body: { appendChild: () => {} },
      createElement: () => ({ style: {} }),
      addEventListener: () => {},
    },
    window: {}, console, Date, Math, Promise,
    setTimeout, clearTimeout, setInterval: () => 0,
    GM_setValue: () => {},
  };
  vm.createContext(ctx);
  vm.runInContext(
    iife + "\n;globalThis.__fluxD = FluxAutoFeedAdapter;", ctx);
  const d = ctx.__fluxD._test.diag();
  assert.equal(d.build, "2026-11-07e");
  assert.equal(d.mode, 9, "mode=" + d.mode);
  assert.equal(d.isFluxSite, true);
  assert.equal(d.hasForm, true);
  assert.equal(d.hasFileInput, true);
  assert.equal(d.fileCount, 1);
  assert.equal(d.cacheKey, "kk");
});

test("产物：种子填充已从字段 try 中拆出", () => {
  assert.ok(src.includes("async function fillTorrentSafely"));
  assert.ok(src.includes("function showDiagPanel"));
  assert.ok(src.includes("fields: "));
  // 种子调用必须出现在 fetchProfile 之前
  const seed = src.indexOf("var seedOk = await fillTorrentSafely");
  const profile = src.indexOf("var profile = await fetchProfile", seed);
  assert.ok(seed > 0 && profile > seed, "种子必须先于字段填充执行");
});

test("真实 handleTarget：字段 API 全挂也不影响种子填充", async () => {
  const vm = require("node:vm");
  const start = src.indexOf("var FLUX_SITE_HOSTS = ");
  const end = src.indexOf("\n})();", start);
  const iife = src.slice(start, end + "\n})();".length);
  const raw = {
    name: "T.2026", torrent_b64_key: "k1",
    torrent_url: "https://src.example/x",
  };
  const input = {
    files: null,
    dispatched: [],
    dispatchEvent(e) { this.dispatched.push(e.type); },
    __reactProps$x: { onChange() {} },
  };
  const form = {
    __reactProps$form: {},
    querySelector(sel) {
      return sel.indexOf('type="file"') >= 0 ||
        sel.indexOf('accept*=".torrent"') >= 0 ? input : null;
    },
  };
  const fakeEl = () => ({
    style: {}, textContent: "", className: "", id: "",
    parentNode: null,
    setAttribute() {}, appendChild() {}, removeChild() {},
    addEventListener() {},
  });
  class FakeBlob {
    constructor(parts) { this.parts = parts; }
  }
  class FakeFile extends FakeBlob {
    constructor(parts, name) { super(parts); this.name = name; }
  }
  class FakeDT {
    constructor() {
      this._l = [];
      const self = this;
      this.items = { add(f) { self._l.push(f); } };
    }
    get files() { return this._l; }
  }
  const ctx = {
    FluxAutoFeedCore: core,
    raw_info: {},
    stringToDict: () => raw,
    site_url: "https://www.hxpt.org/upload#separator#x",
    document: {
      querySelector: (sel) => sel === "#upload-form" ? form : null,
      createElement: fakeEl,
      head: fakeEl(), body: fakeEl(), documentElement: fakeEl(),
      cookie: "", addEventListener() {},
    },
    location: {
      href: "https://www.hxpt.org/upload#separator#x",
      pathname: "/upload", hostname: "www.hxpt.org",
      origin: "https://www.hxpt.org",
    },
    window: { File: FakeFile },
    GM_getValue: (k) => k === "flux_torrent_b64"
      ? JSON.stringify({ key: "k1", b64: "eA==", at: Date.now() })
      : undefined,
    GM_setValue: () => {},
    fetch: () => Promise.reject(new Error("site-profile 挂了")),
    build_blob_from_torrent: (bin, ann, site, type, cb) =>
      cb({ data: new FakeBlob([new Uint8Array([100])]) }),
    atob: (s) => Buffer.from(s, "base64").toString("binary"),
    DataTransfer: FakeDT,
    Event: class { constructor(t) { this.type = t; } },
    setTimeout: () => 0,
    clearTimeout: () => {},
    setInterval: () => 0,
    Date, Math, Promise, console, Uint8Array,
    encodeURIComponent, decodeURIComponent, URL,
  };
  vm.createContext(ctx);
  vm.runInContext(
    iife + "\n;globalThis.__fluxT = FluxAutoFeedAdapter;", ctx);
  await ctx.__fluxT._test.handleTarget();
  assert.equal(input.files.length, 1, "字段挂了种子也必须填");
  assert.equal(input.files[0].name, "T.2026.torrent");
  assert.ok(input.dispatched.includes("change"));
});

(async function run() {
  for (const t of tests) {
    try {
      await t.fn();
      console.log("ok   " + t.name);
    } catch (err) {
      console.error("FAIL " + t.name);
      throw err;
    }
  }
  console.log("all patch tests passed");
})().catch((err) => {
  console.error(err);
  process.exitCode = 1;
});
