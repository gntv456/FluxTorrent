/* FluxTorrent auto_feed 适配层：站点识别、源站面板、目标站自动填充。
 *
 * 注入位置：上游 `function auto_feed()` 之前；`auto_feed()` 开头拦截
 * FluxTorrent 页面，避免走 NexusPHP 的解析分支。
 * 依赖上游作用域里的：used_site_info / site_order / used_common_sites /
 * set_jump_href / dictToString / stringToDict / addTorrent / raw_info /
 * site_url / location。
 */

var FLUX_SITE_HOSTS = ["ft.ptang.top", "fluxtorrent.ptang.top",
  "mfwg.ptang.top"];

function fluxIsFluxHost(host) {
  return FLUX_SITE_HOSTS.indexOf(host) >= 0;
}

function fluxIsTorrentHost() {
  try {
    var meta = document.querySelector('meta[name="generator"]');
    if (meta && /flux/i.test(meta.getAttribute("content") || "")) return true;
    var host = (location.hostname || "").toLowerCase();
    return fluxIsFluxHost(host) || host === "localhost" ||
      host === "127.0.0.1" || host === "::1";
  } catch (e) {
    return false;
  }
}

function fluxIsTorrentSite() {
  if (!fluxIsTorrentHost()) return false;
  try {
    var meta = document.querySelector('meta[name="generator"]');
    if (meta && /flux/i.test(meta.getAttribute("content") || "")) return true;
    var host = (location.hostname || "").toLowerCase();
    if (fluxIsFluxHost(host)) return true;
    return !!document.querySelector(
      "#upload-form, article.td-page, .td-page",
    );
  } catch (e) {
    return false;
  }
}

function fluxSiteMode() {
  if (!fluxIsTorrentSite()) return null;
  var path = location.pathname || "";
  var href = location.href || "";
  if (/^\/torrent\/\d+\/?$/.test(path)) return 8;
  if (/^\/upload\/?$/.test(path)) {
    return href.indexOf("#separator#") >= 0 ? 9 : null;
  }
  return null;
}

var FluxAutoFeedAdapter = (function () {
  "use strict";

  var core = FluxAutoFeedCore;
  var state = {
    key: "",
    raw: null,
    mode: 1,
    links: null,
    linksMode: null,
    linkError: "",
    linkAt: 0,
    refreshing: false,
    profile: null,
    profileAt: 0,
    sections: {},
    pill: null,
    panel: null,
    sitesEl: null,
    statusEl: null,
    filter: null,
    dup: null,
    toast: null,
    toastTimer: 0,
    styles: false,
    expanded: false,
    b64Tried: false,
    b64Busy: false,
  };

  // ------------------------------------------------------------- 入口
  function main() {
    var isUpload = /^\/upload\/?$/.test(location.pathname || "");
    var key = (location.pathname || "") + (isUpload ? location.hash : "");
    if (key === state.key) return;
    state.key = key;
    removePanel();
    state.raw = null;
    state.links = null;
    state.linkError = "";
    state.linkAt = 0;
    state.refreshing = false;
    state.b64Tried = false;
    state.b64Busy = false;
    var mode = fluxSiteMode();
    if (mode === 8) {
      void handleSource();
    } else if (mode === 9) {
      void handleTarget();
    }
  }

  function torrentId() {
    var m = (location.pathname || "").match(/^\/torrent\/(\d+)/);
    return m ? m[1] : "";
  }

  // ------------------------------------------------------------- 源站
  async function handleSource() {
    var id = torrentId();
    if (!id) return;
    mountPill("转载", "正在读取种子信息…", true, false);
    try {
      var profile = await fetchProfile();
      var agg = await apiGet(
        "/api/v1/torrents/" + encodeURIComponent(id) + "/aggregate",
      );
      var raw = core.buildRawInfo(agg, profile, location.href);
      try {
        raw.torrent_url = await fetchDownloadLink(id);
        state.linkAt = Date.now();
      } catch (e) {
        state.linkError = errText(e);
      }
      state.raw = raw;
      mountPill(
        "转载",
        state.linkError ? "种子文件不可用：" + state.linkError : "选择目标站",
        false,
        false,
      );
    } catch (e) {
      mountPill("转载不可用", errText(e), false, true);
      console.warn("[flux-auto-feed] source failed:", e);
    }
  }

  function openPanel() {
    if (!state.raw) return;
    ensureStyles();
    if (!state.panel) buildPanel();
    state.expanded = true;
    if (state.pill) state.pill.style.display = "none";
    state.panel.style.display = "flex";
    var stale = state.linkAt &&
      Date.now() - state.linkAt > 25 * 60 * 1000;
    if (stale) {
      setStatus("正在刷新种子直链…", false);
      void refreshLink();
      if (state.filter) setTimeout(function () { state.filter.focus(); }, 30);
      return;
    }
    if (!state.b64Tried && !state.b64Busy) {
      state.b64Tried = true;
      setStatus("正在准备种子文件…", false);
      void prepareTorrentB64().then(function () {
        if (!state.expanded) return;
        ensureLinks(state.mode);
        if (state.linkError) {
          setStatus("种子文件不可用：" + state.linkError, true);
        }
      });
      if (state.filter) setTimeout(function () { state.filter.focus(); }, 30);
      return;
    }
    if (!state.links) ensureLinks(state.mode);
    if (state.linkError) setStatus("种子文件不可用：" + state.linkError, true);
    if (state.filter) setTimeout(function () { state.filter.focus(); }, 30);
  }

  /* 源站同源拉一次 .torrent，清洗掉 announce/passkey 后存 GM 存储；
   * 载荷只带 key，目标站重建时用自己的 announce。这样目标站不需要跨站
   * 拉 token 直链（Cloudflare/无存档时会拿到 JSON/HTML 而报“加载失败”）。 */
  async function prepareTorrentB64() {
    var raw = state.raw;
    if (!raw || !raw.torrent_url) return;
    if (typeof build_blob_from_torrent !== "function" ||
      typeof GM_setValue !== "function") {
      return;
    }
    if (state.b64Busy) return;
    state.b64Busy = true;
    try {
      var res = await fetch(raw.torrent_url, {
        credentials: "same-origin",
        cache: "no-store",
      });
      if (!res.ok) {
        var msg = "HTTP " + res.status;
        try {
          var j = await res.json();
          if (j && j.message) msg = j.message;
        } catch (e) {
          /* keep HTTP status */
        }
        throw new Error(msg);
      }
      var ab = await res.arrayBuffer();
      if (ab.byteLength > 8 * 1024 * 1024) return;
      var bin = bytesToBin(new Uint8Array(ab));
      if (bin.indexOf("8:announce") < 0) {
        throw new Error("返回内容不是 .torrent");
      }
      if (bin.indexOf("7:privatei1e") < 0) {
        throw new Error("种子未标记私有");
      }
      var safe = await buildSafeTorrent(bin);
      if (!safe) throw new Error("种子清洗失败");
      var key = String(Date.now()) + "-" +
        Math.random().toString(36).slice(2, 8);
      GM_setValue("flux_torrent_b64", JSON.stringify({
        key: key, b64: safe, at: Date.now(),
      }));
      raw.torrent_b64_key = key;
      state.linkError = "";
    } catch (e) {
      state.linkError = "种子文件读取失败：" + errText(e);
      raw.torrent_url = "";
    } finally {
      state.b64Busy = false;
    }
  }

  function bytesToBin(bytes) {
    var bin = "";
    for (var i = 0; i < bytes.length; i += 0x8000) {
      bin += String.fromCharCode.apply(
        null, bytes.subarray(i, i + 0x8000),
      );
    }
    return bin;
  }

  function bytesToB64(bytes) {
    return btoa(bytesToBin(bytes));
  }

  function buildSafeTorrent(bin) {
    return new Promise(function (resolve, reject) {
      var done = false;
      var timer = setTimeout(function () {
        if (!done) reject(new Error("种子清洗超时"));
      }, 4000);
      function finish(fn, arg) {
        done = true;
        clearTimeout(timer);
        fn(arg);
      }
      try {
        build_blob_from_torrent(
          bin, "https://placeholder.invalid/announce",
          "FluxTorrent", "application/x-bittorrent",
          function (data) {
            var blob = data && data.data;
            if (!blob || typeof blob.arrayBuffer !== "function") {
              finish(reject, new Error("种子清洗无输出"));
              return;
            }
            blob.arrayBuffer().then(function (buf) {
              finish(resolve, bytesToB64(new Uint8Array(buf)));
            }).catch(function (e) {
              finish(reject, e);
            });
          },
        );
      } catch (e) {
        finish(reject, e);
      }
    });
  }

  // 直链 30 分钟过期：面板久开后重新签发（不预扣费，只查下载资格）
  async function refreshLink() {
    if (state.refreshing || !state.raw) return;
    state.refreshing = true;
    state.b64Tried = true;
    try {
      state.raw.torrent_url = await fetchDownloadLink(torrentId());
      state.raw.torrent_b64_key = "";
      state.linkError = "";
      state.linkAt = Date.now();
      await prepareTorrentB64();
      state.links = null;
      if (state.expanded) ensureLinks(state.mode);
      if (state.linkError) {
        setStatus("种子文件不可用：" + state.linkError, true);
      }
    } catch (e) {
      state.linkError = errText(e);
      setStatus("种子文件不可用：" + state.linkError, true);
    } finally {
      state.refreshing = false;
    }
  }

  function closePanel() {
    state.expanded = false;
    if (state.panel) state.panel.style.display = "none";
    if (state.pill) state.pill.style.display = "";
  }

  function buildHrefs(raw, mode) {
    if (typeof set_jump_href !== "function") {
      throw new Error("上游 set_jump_href 未就绪");
    }
    var keys = allSiteKeys();
    var host = document.createElement("div");
    host.style.display = "none";
    var saved = [];
    document.body.appendChild(host);
    keys.forEach(function (k) {
      var old = document.getElementById(k);
      if (old) {
        saved.push([old, old.id]);
        old.id = "";
      }
      var a = document.createElement("a");
      a.id = k;
      host.appendChild(a);
    });
    var prev = null;
    var hrefs = {};
    try {
      prev = raw_info;
      raw_info = raw;
      set_jump_href(raw, mode);
      keys.forEach(function (k) {
        var a = document.getElementById(k);
        if (a && a.getAttribute("href")) {
          hrefs[k] = a.getAttribute("href");
        }
      });
    } finally {
      raw_info = prev;
      if (host.parentNode) host.parentNode.removeChild(host);
      saved.forEach(function (pair) {
        pair[0].id = pair[1];
      });
    }
    return hrefs;
  }

  function allSiteKeys() {
    if (typeof used_site_info === "undefined" || !used_site_info) return [];
    var order = (typeof site_order !== "undefined" && site_order) || [];
    var out = [];
    order.forEach(function (k) {
      if (used_site_info[k] && used_site_info[k].enable) out.push(k);
    });
    Object.keys(used_site_info).forEach(function (k) {
      if (used_site_info[k] && used_site_info[k].enable &&
        out.indexOf(k) < 0) {
        out.push(k);
      }
    });
    return out;
  }

  function ensureLinks(mode) {
    state.mode = mode;
    state.links = null;
    state.sitesEl.textContent = "";
    setStatus("正在准备链接…", false);
    var hrefs;
    try {
      hrefs = buildHrefs(state.raw, mode);
    } catch (e) {
      setStatus("生成链接失败：" + errText(e), true);
      return;
    }
    state.links = hrefs;
    state.linksMode = mode;
    renderSites(hrefs);
    var n = countLinks(hrefs);
    setStatus(
      mode === 1
        ? "共 " + n + " 个站点；点击跳到对方发布页并自动填充"
        : "查重模式：点击站点打开对方搜索页",
      false,
    );
    if (state.filter && state.filter.value) applyFilter(state.filter.value);
  }

  function countLinks(hrefs) {
    var n = 0;
    Object.keys(hrefs).forEach(function (k) {
      if (k !== "FluxTorrent" && hrefs[k]) n++;
    });
    return n;
  }

  function renderSites(hrefs) {
    var common = (typeof used_common_sites !== "undefined" &&
      used_common_sites) || [];
    var keys = allSiteKeys().filter(function (k) {
      return k !== "FluxTorrent" && hrefs[k];
    });
    var ordered = [];
    common.forEach(function (k) {
      if (keys.indexOf(k) >= 0 && ordered.indexOf(k) < 0) ordered.push(k);
    });
    keys.forEach(function (k) {
      if (ordered.indexOf(k) < 0) ordered.push(k);
    });
    var frag = document.createDocumentFragment();
    ordered.forEach(function (k) {
      var a = document.createElement("a");
      a.className = "flux-af-site";
      a.href = hrefs[k];
      a.target = "_blank";
      a.rel = "noreferrer noopener";
      a.textContent = k;
      a.title = k;
      a.setAttribute("data-flux-site", k);
      frag.appendChild(a);
    });
    state.sitesEl.appendChild(frag);
  }

  function applyFilter(q) {
    var needle = String(q || "").trim().toLowerCase();
    var nodes = state.sitesEl.children;
    for (var i = 0; i < nodes.length; i++) {
      var k = nodes[i].getAttribute("data-flux-site") || "";
      nodes[i].style.display = !needle || k.toLowerCase().indexOf(needle) >= 0
        ? ""
        : "none";
    }
  }

  // ------------------------------------------------------------- 目标站
  async function handleTarget() {
    var raw;
    try {
      raw = typeof stringToDict === "function"
        ? stringToDict(site_url)
        : core.decodePayload(location.href);
    } catch (e) {
      raw = core.decodePayload(location.href);
    }
    if (!raw || !raw.name) {
      toast("转载载荷无法解析", true);
      return;
    }
    var form = await waitUntil(function () {
      return document.querySelector("#upload-form");
    }, 15000);
    if (!form) {
      toast("未找到发布表单", true);
      return;
    }
    // SSR 表单先到、React 水合后才是受控组件：未水合就写值会被首帧
    // state 覆盖。fiber/props 键是 React 已接管该节点的可靠信号。
    await waitForHydration(form, 8000);
    toast("正在自动填充…", false);
    try {
      var profile = await fetchProfile();
      var catSel = form.querySelector('select[name="category_id"]') ||
        findByRowLabel(form, "category_id");
      await waitUntil(function () {
        return catSel && catSel.options.length > 0;
      }, 8000);
      var targetCat = core.pickCategory(profile.categories, raw);
      if (targetCat === null && catSel) targetCat = Number(catSel.value);
      var sec = await fetchSections(targetCat);
      var plan = core.planTargetFill(raw, profile, sec);
      if (plan.category_id !== null && catSel) {
        setValue(catSel, String(plan.category_id));
        await waitUntil(function () {
          return !hasPlanSections(plan) ||
            findControls(form, plan, sec).length > 0;
        }, 8000);
        await sleep(120);
      }
      await applyFields(form, plan);
      await applyTags(form, plan.tags);
      await applySections(form, plan, sec);
      if (raw.torrent_url) {
        if (typeof addTorrent === "function") {
          addTorrent(
            raw.torrent_url,
            raw.torrent_name || raw.name,
            "FluxTorrent",
            (location.origin || "") + "/announce",
          );
        } else {
          toast("未找到上游 addTorrent，请手动选择种子文件", true);
          return;
        }
      } else {
        toast("载荷里没有种子直链，请手动下载后选择 .torrent", true);
        return;
      }
      toast("已自动填充，请核对后提交", false);
    } catch (e) {
      toast("自动填充失败：" + errText(e), true);
      console.warn("[flux-auto-feed] target failed:", e);
    }
  }

  function hasPlanSections(plan) {
    return Object.keys(plan.sections || {}).length > 0;
  }

  async function waitForHydration(form, timeout) {
    var ready = await waitUntil(function () {
      var keys = Object.keys(form);
      for (var i = 0; i < keys.length; i++) {
        if (keys[i].indexOf("__reactProps$") === 0 ||
          keys[i].indexOf("__reactFiber$") === 0) {
          return true;
        }
      }
      return false;
    }, timeout);
    if (!ready) await sleep(600);
  }

  async function applyFields(form, plan) {
    var f = plan.fields || {};
    await setNamed(form, "name", f.name);
    await setNamed(form, "small_descr", f.small_descr);
    await setNamed(form, "imdb", f.imdb);
    await setNamed(form, "douban", f.douban);
    await setNamed(form, "ptgen_url", f.ptgen);
    await setNamed(form, "descr", f.descr);
    await setNamed(form, "mediainfo", f.mediainfo);
    await setNamed(form, "poster", f.poster);
  }

  async function setNamed(form, name, value) {
    if (!value) return;
    var sel = '[name="' + name + '"]';
    var el = form.querySelector(sel) || findByRowLabel(form, name);
    if (!el) {
      el = await waitUntil(function () {
        return form.querySelector(sel) || findByRowLabel(form, name);
      }, 1500);
    }
    if (el) setValue(el, value);
  }

  // 旧镜像没有 JSX 里的 name：按行标题兜底（三语）。新版 name 优先。
  var FLUX_ROW_LABELS = {
    name: ["标题", "Title", "標題"],
    small_descr: ["副标题", "Subtitle", "副標題"],
    imdb: ["IMDb 链接", "IMDb link", "IMDb 連結"],
    douban: ["豆瓣 / TMDB", "Douban / TMDB"],
    tmdb: ["豆瓣 / TMDB", "Douban / TMDB"],
    ptgen_url: ["PT-Gen"],
    descr: ["资源简介", "Description", "資源簡介"],
    mediainfo: ["MediaInfo"],
    poster: ["封面图 URL", "Cover image URL", "封面圖 URL"],
    category_id: ["分类", "Category", "分類"],
  };

  function findByRowLabel(form, name) {
    var labels = FLUX_ROW_LABELS[name];
    if (!labels) return null;
    var index = name === "tmdb" ? 1 : 0;
    var tag = "input";
    if (name === "descr" || name === "mediainfo") tag = "textarea";
    if (name === "category_id") tag = "select";
    var rows = form.querySelectorAll("tr");
    for (var i = 0; i < rows.length; i++) {
      var head = rows[i].querySelector(".rowhead");
      if (!head) continue;
      var label = (head.textContent || "").trim();
      if (labels.indexOf(label) < 0) continue;
      var els = rows[i].querySelectorAll(tag);
      if (els[index]) return els[index];
    }
    return null;
  }

  function setValue(el, value) {
    var proto;
    if (el.tagName === "TEXTAREA") {
      proto = HTMLTextAreaElement.prototype;
    } else if (el.tagName === "SELECT") {
      proto = HTMLSelectElement.prototype;
    } else {
      proto = HTMLInputElement.prototype;
    }
    var desc = Object.getOwnPropertyDescriptor(proto, "value");
    if (desc && desc.set) desc.set.call(el, value);
    else el.value = value;
    el.dispatchEvent(new Event("input", { bubbles: true }));
    if (el.tagName === "SELECT") {
      el.dispatchEvent(new Event("change", { bubbles: true }));
    }
  }

  function findControls(form, plan, sec) {
    var kinds = (sec && sec.kinds) || [];
    var found = [];
    kinds.forEach(function (k) {
      if (!Object.prototype.hasOwnProperty.call(plan.sections, k.kind)) {
        return;
      }
      var el = findByAria(form, k.label);
      if (el) found.push(el);
    });
    return found;
  }

  async function applySections(form, plan, sec) {
    var kinds = (sec && sec.kinds) || [];
    var tasks = [];
    kinds.forEach(function (k) {
      if (Object.prototype.hasOwnProperty.call(plan.sections, k.kind)) {
        tasks.push({ meta: k, value: plan.sections[k.kind] });
      }
    });
    if (!tasks.length) return;
    await waitUntil(function () {
      return tasks.some(function (t) {
        return !!findByAria(form, t.meta.label);
      });
    }, 5000);
    for (var i = 0; i < tasks.length; i++) {
      var t = tasks[i];
      var el = findByAria(form, t.meta.label);
      if (!el) {
        el = await waitUntil(function () {
          return findByAria(form, t.meta.label);
        }, 1500);
      }
      if (!el) continue;
      if (el.tagName === "SELECT") {
        if (hasOption(el, t.value)) setValue(el, t.value);
      } else if (el.tagName === "INPUT" || el.tagName === "TEXTAREA") {
        setValue(el, t.value);
      }
    }
  }

  function findByAria(root, label) {
    var nodes = root.querySelectorAll("[aria-label]");
    for (var i = 0; i < nodes.length; i++) {
      if (nodes[i].getAttribute("aria-label") === label) return nodes[i];
    }
    return null;
  }

  // ------------------------------------------------------------- 标签
  var FLUX_TAG_ALIASES = {
    "中字": ["中字", "中文字幕", "简繁中字", "簡繁中字"],
    "国语": ["国语", "國語", "国配", "國配", "普通话", "普通話"],
    "粤语": ["粤语", "粵語", "广东话", "廣東話"],
    "英字": ["英字", "英文字幕", "英语字幕", "英語字幕"],
    "DIY": ["diy"],
    "禁转": ["禁转", "禁轉"],
    "首发": ["首发", "首發"],
    "独家": ["独家", "獨家"],
    "合集": ["合集", "全集", "pack"],
    "完结": ["完结", "完結", "complete"],
    "HDR": ["hdr", "hdr10", "hdr10+", "hdr10plus"],
    "杜比视界": ["杜比视界", "dolby vision", "dv"],
    "特效字幕": ["特效字幕", "特效"],
    "官种": ["官种", "官種", "官方"],
    "官方": ["官方", "官种", "官種"],
  };

  function findRowByHead(form, labels) {
    var rows = form.querySelectorAll("tr");
    for (var i = 0; i < rows.length; i++) {
      var head = rows[i].querySelector(".rowhead");
      if (!head) continue;
      var t = (head.textContent || "").trim();
      if (labels.indexOf(t) >= 0) return rows[i];
    }
    return null;
  }

  function tagMatches(name, desired, exactOnly) {
    var n = core.norm(name);
    var d = core.norm(desired);
    if (!n || !d) return false;
    if (n === d) return true;
    var aliases = FLUX_TAG_ALIASES[desired] || [];
    for (var i = 0; i < aliases.length; i++) {
      var a = core.norm(aliases[i]);
      if (!a) continue;
      if (n === a) return true;
      if (!exactOnly && n.indexOf(a) >= 0) return true;
    }
    return false;
  }

  async function applyTags(form, tags) {
    if (!tags || !tags.length) return;
    var rowLabels = ["标签", "Tags", "標籤"];
    var row = findRowByHead(form, rowLabels);
    if (!row) {
      await waitUntil(function () {
        return !!findRowByHead(form, rowLabels);
      }, 5000);
      row = findRowByHead(form, rowLabels);
    }
    if (!row) return;
    var labels = row.querySelectorAll("label");
    var matched = {};
    function pass(exactOnly) {
      for (var i = 0; i < labels.length; i++) {
        var input = labels[i].querySelector('input[type="checkbox"]');
        if (!input || input.checked) continue;
        var name = (labels[i].textContent || "").trim();
        if (!name || core.isSkippedTag(name)) continue;
        for (var j = 0; j < tags.length; j++) {
          if (matched[tags[j]]) continue;
          if (tagMatches(name, tags[j], exactOnly)) {
            input.click();
            matched[tags[j]] = true;
            break;
          }
        }
      }
    }
    pass(true);
    pass(false);
  }

  function hasOption(sel, value) {
    for (var i = 0; i < sel.options.length; i++) {
      if (sel.options[i].value === String(value)) return true;
    }
    return false;
  }

  async function fetchSections(catId) {
    var key = String(catId || 0);
    if (state.sections[key]) return state.sections[key];
    var data = await apiGet(
      "/api/v1/section-dict?category_id=" + encodeURIComponent(key),
    );
    state.sections[key] = data || {};
    return state.sections[key];
  }

  // ------------------------------------------------------------- 网络
  async function fetchProfile() {
    var now = Date.now();
    if (state.profile && now - state.profileAt < 60000) {
      return state.profile;
    }
    var data = await apiGet("/api/v1/site-profile");
    state.profile = data || {};
    state.profileAt = now;
    return state.profile;
  }

  async function apiGet(path) {
    var res = await fetch(path, {
      credentials: "same-origin",
      cache: "no-store",
      headers: { Accept: "application/json" },
    });
    var body = null;
    try {
      body = await res.json();
    } catch (e) {
      body = null;
    }
    if (!res.ok || !body || body.code !== 0) {
      throw new Error(body && body.message ? body.message : "HTTP " + res.status);
    }
    return body.data;
  }

  function csrfCookie() {
    var m = document.cookie.match(/(?:^|;\s*)flux_csrf=([^;]+)/);
    return m ? decodeURIComponent(m[1]) : null;
  }

  async function csrfToken() {
    var t = csrfCookie();
    if (t) return t;
    try {
      await fetch("/api/v1/me/csrf", {
        credentials: "same-origin",
        cache: "no-store",
      });
    } catch (e) {
      /* 拿不到就让后端回 403，错误会显示在面板 */
    }
    return csrfCookie();
  }

  async function fetchDownloadLink(id) {
    var token = await csrfToken();
    var headers = { Accept: "application/json" };
    if (token) headers["x-csrf-token"] = token;
    var res = await fetch(
      "/api/v1/torrents/" + encodeURIComponent(id) + "/download-link",
      {
        method: "POST",
        credentials: "same-origin",
        cache: "no-store",
        headers: headers,
      },
    );
    var body = null;
    try {
      body = await res.json();
    } catch (e) {
      body = null;
    }
    if (!res.ok || !body || body.code !== 0) {
      throw new Error(body && body.message ? body.message : "HTTP " + res.status);
    }
    var url = body.data && body.data.download_url;
    if (!url) throw new Error("下载直链为空");
    return url.indexOf("http") === 0 ? url : location.origin + url;
  }

  // ------------------------------------------------------------- UI
  function ensureStyles() {
    if (state.styles) return;
    state.styles = true;
    var css = [
      ".flux-af-pill{position:fixed;right:16px;bottom:88px;z-index:2147483000;",
      "display:inline-flex;align-items:center;gap:6px;padding:9px 16px;",
      "border-radius:999px;background:#111827;color:#f9fafb;font:600 14px/1",
      "system-ui,-apple-system,'Segoe UI',sans-serif;cursor:pointer;",
      "box-shadow:0 8px 24px rgba(0,0,0,.28);border:1px solid",
      "rgba(255,255,255,.18);user-select:none}",
      ".flux-af-pill:hover{border-color:#f59e0b;color:#fbbf24}",
      ".flux-af-pill[data-busy=true]{opacity:.65;cursor:progress}",
      ".flux-af-pill[data-dead=true]{opacity:.55;cursor:not-allowed}",
      ".flux-af-panel{position:fixed;right:16px;bottom:88px;z-index:2147483000;",
      "display:none;flex-direction:column;width:360px;max-width:calc(100vw -",
      "32px);max-height:62vh;border-radius:14px;background:#111827;color:#f9fafb;",
      "border:1px solid rgba(255,255,255,.18);box-shadow:0 18px 48px",
      "rgba(0,0,0,.38);font:13px/1.45 system-ui,-apple-system,'Segoe UI',",
      "sans-serif;overflow:hidden}",
      ".flux-af-head{display:flex;align-items:center;justify-content:space-between;",
      "gap:8px;padding:10px 12px;border-bottom:1px solid rgba(255,255,255,.12);",
      "background:rgba(255,255,255,.04)}",
      ".flux-af-title{font-weight:700;color:#fbbf24}",
      ".flux-af-close{border:0;background:transparent;color:#9ca3af;",
      "font-size:18px;line-height:1;cursor:pointer;padding:2px 6px}",
      ".flux-af-close:hover{color:#f9fafb}",
      ".flux-af-body{display:flex;flex-direction:column;gap:8px;padding:10px",
      "12px 12px;overflow:auto}",
      ".flux-af-row{display:flex;align-items:center;gap:8px;flex-wrap:wrap}",
      ".flux-af-filter{flex:1;min-width:120px;padding:6px 9px;border-radius:8px;",
      "border:1px solid rgba(255,255,255,.18);background:rgba(255,255,255,.06);",
      "color:#f9fafb;font-size:13px;outline:none}",
      ".flux-af-filter:focus{border-color:#f59e0b}",
      ".flux-af-dup{display:inline-flex;align-items:center;gap:5px;",
      "color:#d1d5db;cursor:pointer;user-select:none;white-space:nowrap}",
      ".flux-af-status{color:#9ca3af;font-size:12px}",
      ".flux-af-status[data-err=true]{color:#fca5a5}",
      ".flux-af-sites{display:flex;flex-wrap:wrap;gap:6px;align-content:",
      "flex-start}",
      ".flux-af-site{display:inline-flex;align-items:center;padding:4px 9px;",
      "border-radius:7px;background:rgba(255,255,255,.08);color:#e5e7eb;",
      "text-decoration:none;font-size:12px;border:1px solid transparent}",
      ".flux-af-site:hover{background:rgba(245,158,11,.18);",
      "border-color:#f59e0b;color:#fde68a}",
      ".flux-af-foot{color:#6b7280;font-size:11px;border-top:1px solid",
      "rgba(255,255,255,.1);padding-top:6px}",
      ".flux-af-tool{color:#fbbf24;text-decoration:underline;cursor:pointer}",
      ".flux-af-toast{position:fixed;left:50%;top:18px;transform:translateX(-50%);",
      "z-index:2147483001;display:none;max-width:min(560px,calc(100vw - 32px));",
      "padding:10px 16px;border-radius:10px;background:#065f46;color:#ecfdf5;",
      "font:600 13px/1.4 system-ui,-apple-system,'Segoe UI',sans-serif;",
      "box-shadow:0 10px 30px rgba(0,0,0,.3)}",
      ".flux-af-toast[data-err=true]{background:#7f1d1d;color:#fee2e2}",
      "@media (max-width:640px){.flux-af-panel{right:12px;left:12px;",
      "width:auto;bottom:78px;max-height:56vh}.flux-af-pill{right:12px;",
      "bottom:78px}}",
    ].join("");
    var style = document.createElement("style");
    style.id = "flux-af-style";
    style.textContent = css;
    document.head.appendChild(style);
  }

  function mountPill(label, title, busy, dead) {
    ensureStyles();
    var pill = state.pill;
    if (!pill) {
      pill = document.createElement("div");
      pill.id = "flux-af-pill";
      pill.className = "flux-af-pill";
      pill.textContent = label;
      pill.addEventListener("click", function () {
        if (pill.getAttribute("data-busy") === "true") {
          toast("正在读取种子信息，请稍候", false);
          return;
        }
        if (pill.getAttribute("data-dead") === "true") return;
        openPanel();
      });
      document.body.appendChild(pill);
      state.pill = pill;
    }
    pill.textContent = label;
    pill.title = title || "";
    pill.setAttribute("data-busy", busy ? "true" : "false");
    pill.setAttribute("data-dead", dead ? "true" : "false");
    pill.style.display = state.expanded ? "none" : "";
  }

  function buildPanel() {
    var panel = document.createElement("div");
    panel.id = "flux-af-panel";
    panel.className = "flux-af-panel";

    var head = document.createElement("div");
    head.className = "flux-af-head";
    var title = document.createElement("span");
    title.className = "flux-af-title";
    title.textContent = "转载到…";
    var close = document.createElement("button");
    close.type = "button";
    close.className = "flux-af-close";
    close.title = "收起";
    close.textContent = "×";
    close.addEventListener("click", closePanel);
    head.appendChild(title);
    head.appendChild(close);

    var body = document.createElement("div");
    body.className = "flux-af-body";

    var row = document.createElement("div");
    row.className = "flux-af-row";
    var filter = document.createElement("input");
    filter.type = "text";
    filter.className = "flux-af-filter";
    filter.placeholder = "筛选站点…";
    filter.addEventListener("input", function () {
      applyFilter(filter.value);
    });
    var dupLabel = document.createElement("label");
    dupLabel.className = "flux-af-dup";
    var dup = document.createElement("input");
    dup.type = "checkbox";
    dupLabel.appendChild(dup);
    dupLabel.appendChild(document.createTextNode("查重"));
    dup.addEventListener("change", function () {
      ensureLinks(dup.checked ? 0 : 1);
    });
    row.appendChild(filter);
    row.appendChild(dupLabel);

    var status = document.createElement("div");
    status.className = "flux-af-status";

    var sites = document.createElement("div");
    sites.className = "flux-af-sites";

    var extra = document.createElement("div");
    extra.className = "flux-af-foot";
    var raw = state.raw || {};
    var tagText = core.desiredTags(raw).join(" ");
    var recog = document.createElement("span");
    recog.textContent =
      "MediaInfo：" +
      (core.text(raw.full_mediainfo).trim() ? "已识别" : "未识别") +
      " · 标签：" + (tagText || "未识别");
    extra.appendChild(recog);
    var pg = ptgenHref(raw);
    if (pg) {
      extra.appendChild(document.createTextNode(" · "));
      var a = document.createElement("a");
      a.className = "flux-af-tool";
      a.href = pg;
      a.target = "_blank";
      a.rel = "noreferrer noopener";
      a.textContent = "PTGen";
      extra.appendChild(a);
    }

    var foot = document.createElement("div");
    foot.className = "flux-af-foot";
    foot.textContent = "点击站点 → 跳到对方发布页并自动填充；查重模式只打开搜索页。";

    body.appendChild(row);
    body.appendChild(status);
    body.appendChild(extra);
    body.appendChild(sites);
    body.appendChild(foot);
    panel.appendChild(head);
    panel.appendChild(body);
    document.body.appendChild(panel);

    state.panel = panel;
    state.sitesEl = sites;
    state.statusEl = status;
    state.filter = filter;
    state.dup = dup;
  }

  function ptgenHref(raw) {
    if (!raw) return "";
    var id = core.imdbId(raw.url);
    if (id) return "https://doubaninfo.com/#ptgen?url=" + id;
    var m = core.text(raw.dburl).match(/subject\/(\d+)/);
    return m ? "https://doubaninfo.com/#ptgen?url=" + m[1] : "";
  }

  function setStatus(msg, isErr) {
    if (!state.statusEl) return;
    state.statusEl.textContent = msg || "";
    state.statusEl.setAttribute("data-err", isErr ? "true" : "false");
  }

  function removePanel() {
    if (state.panel && state.panel.parentNode) {
      state.panel.parentNode.removeChild(state.panel);
    }
    if (state.pill && state.pill.parentNode) {
      state.pill.parentNode.removeChild(state.pill);
    }
    state.panel = null;
    state.pill = null;
    state.sitesEl = null;
    state.statusEl = null;
    state.filter = null;
    state.dup = null;
    state.expanded = false;
  }

  function toast(msg, isErr) {
    ensureStyles();
    var el = state.toast;
    if (!el) {
      el = document.createElement("div");
      el.id = "flux-af-toast";
      el.className = "flux-af-toast";
      document.body.appendChild(el);
      state.toast = el;
    }
    el.textContent = msg;
    el.setAttribute("data-err", isErr ? "true" : "false");
    el.style.display = "block";
    clearTimeout(state.toastTimer);
    state.toastTimer = setTimeout(function () {
      el.style.display = "none";
    }, isErr ? 12000 : 5000);
  }

  // ------------------------------------------------------------- 工具
  function errText(e) {
    if (!e) return "未知错误";
    if (e.message) return e.message;
    return String(e);
  }

  function sleep(ms) {
    return new Promise(function (resolve) {
      setTimeout(resolve, ms);
    });
  }

  function waitUntil(fn, timeout) {
    return new Promise(function (resolve) {
      var started = Date.now();
      (function tick() {
        var value = null;
        try {
          value = fn();
        } catch (e) {
          value = null;
        }
        if (value) return resolve(value);
        if (Date.now() - started >= timeout) return resolve(null);
        setTimeout(tick, 150);
      })();
    });
  }

  // ------------------------------------------------------------- 启动
  function boot() {
    main();
    if (fluxIsTorrentHost()) {
      var lastHref = location.href;
      setInterval(function () {
        if (location.href === lastHref) return;
        lastHref = location.href;
        main();
      }, 1200);
    }
  }

  return {
    boot: boot,
    main: main,
    mode: fluxSiteMode,
    isSite: fluxIsTorrentSite,
    _test: {
      buildHrefs: buildHrefs,
      handleTarget: handleTarget,
      applyFields: applyFields,
      setValue: setValue,
      state: state,
    },
  };
})();

if (typeof FluxAutoFeedAdapterBooted === "undefined") {
  var FluxAutoFeedAdapterBooted = true;
  FluxAutoFeedAdapter.boot();
}
