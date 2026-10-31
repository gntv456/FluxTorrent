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

function test(name, fn) {
  try {
    fn();
    console.log("ok   " + name);
  } catch (err) {
    console.error("FAIL " + name);
    throw err;
  }
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
  assert.ok(src.includes("// @version      2.1.1.4.6"));
  assert.ok(!src.includes("@updateURL"));
  assert.ok(!/@require\s+https:\/\/greasyfork\.org\//.test(src));
});

test("头部：FluxTorrent 匹配范围", () => {
  for (const m of [
    "// @match        *://fluxtorrent.ptang.top/*",
    "// @match        *://ft.ptang.top/*",
    "// @match        *://mfwg.ptang.top/*",
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

test("真实 set_jump_href：FluxTorrent 落 /upload 且载荷可解", () => {
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

test("真实 fill_torrent：FluxTorrent 写文件并派发 change", () => {
  const code = extractFn(src, "fill_torrent");
  assert.ok(code, "未找到 fill_torrent");
  let dispatched = 0;
  const input = {
    files: null,
    dispatchEvent: (ev) => {
      dispatched++;
      assert.equal(ev.type, "change");
      return true;
    },
  };
  const env = {
    $: () => {
      throw new Error("Flux 分支不应走 jQuery");
    },
    document: {
      querySelector: (sel) => {
        assert.equal(
          sel,
          '#upload-form input[type="file"][accept*=".torrent"]',
        );
        return input;
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
  assert.equal(dispatched, 1);
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

console.log("all patch tests passed");
