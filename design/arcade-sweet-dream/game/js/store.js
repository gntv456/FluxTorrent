/* 钓鱼玩法 · 存档层
 * localStorage 优先，失败自动降级 sessionStorage → 内存，file:// 直开也不会崩。
 * 写入节流：状态变更只打脏标记，最多每 2s 落盘一次；页面隐藏 / 卸载时强制 flush。
 */
(function (G) {
  'use strict';
  // 站点内嵌时按用户命名空间隔离：FT_NS 由宿主注入（如用户名 / uid）
  const BASE_KEY = 'ft.fishing.save.v1';
  const NS = (G.FT_NS == null ? '' : String(G.FT_NS)).trim();
  const KEY = NS ? BASE_KEY + '.' + NS : BASE_KEY;
  const API = (G.FT_API == null ? '' : String(G.FT_API)).replace(/\/+$/, '');
  const REMOTE = !!API;
  const E = G.FTEngine;

  let backend = null;   // 'local' | 'session' | 'memory'
  let memory = null;    // 内存兜底
  let dirty = false;
  let lastWrite = 0;
  let timer = null;

  function probe(name) {
    try {
      const s = G[name];
      if (!s) return false;
      s.setItem('__ft_probe__', '1');
      s.removeItem('__ft_probe__');
      return true;
    } catch (e) { return false; }
  }

  function pickBackend() {
    if (backend) return backend;
    if (probe('localStorage')) backend = 'local';
    else if (probe('sessionStorage')) backend = 'session';
    else backend = 'memory';
    return backend;
  }

  function readRaw() {
    const b = pickBackend();
    if (b === 'memory') return memory;
    try { return G[b === 'local' ? 'localStorage' : 'sessionStorage'].getItem(KEY); } catch (e) { return null; }
  }

  function writeRaw(text) {
    const b = pickBackend();
    if (b === 'memory') { memory = text; return true; }
    try { G[b === 'local' ? 'localStorage' : 'sessionStorage'].setItem(KEY, text); return true; } catch (e) { return false; }
  }

  function load(now) {
    let parsed = null;
    const raw = readRaw();
    if (raw) {
      try { parsed = JSON.parse(raw); } catch (e) { parsed = null; }
    }
    const state = E.hydrate(parsed, now);
    if (!parsed) save(state);
    return state;
  }

  function serialize(state) {
    return JSON.stringify(state);
  }

  function save(state) {
    const text = serialize(state);
    if (writeRaw(text)) { dirty = false; lastWrite = Date.now(); }
    return text;
  }

  /** 标脏 + 节流落盘 */
  function touch(state, immediate) {
    dirty = true;
    const now = Date.now();
    if (immediate || now - lastWrite > 2000) {
      if (timer) { clearTimeout(timer); timer = null; }
      save(state);
      return;
    }
    if (timer) return;
    timer = setTimeout(() => {
      timer = null;
      if (dirty) save(state);
    }, 2000);
  }

  function flush(state) {
    if (timer) { clearTimeout(timer); timer = null; }
    if (dirty) save(state);
  }

  function reset(now) {
    const fresh = E.defaultState(now);
    save(fresh);
    return fresh;
  }

  function exportText(state) {
    return JSON.stringify(state, null, 2);
  }

  function importText(text, now) {
    let parsed = null;
    try { parsed = JSON.parse(text); } catch (e) { return { ok: false, reason: 'parse' }; }
    if (!parsed || typeof parsed !== 'object') return { ok: false, reason: 'empty' };
    const st = E.hydrate(parsed, now);
    save(st);
    return { ok: true, state: st };
  }

  /* ================= 站点服务端（FT_API 存在时启用） ================= */

  // CSRF（双提交式）：写操作必须带 X-CSRF-Token，且与 flux_csrf cookie 逐字相等。
  // 登录响应/`GET /me/csrf` 都会下发该 cookie；这里先读 cookie，读不到再向
  // 服务端申请一枚（页面开很久或 cookie 过期时仍能继续写）。
  let csrf = '';
  function readCsrfCookie() {
    const m = String(document.cookie || '').match(/(?:^|;\s*)flux_csrf=([^;]*)/);
    return m ? decodeURIComponent(m[1]) : '';
  }
  async function ensureCsrf() {
    if (csrf) return csrf;
    csrf = readCsrfCookie();
    if (csrf) return csrf;
    try {
      const res = await fetch(API + '/me/csrf', { credentials: 'include' });
      const body = await res.json().catch(() => null);
      csrf = (body && body.csrf_token) || readCsrfCookie();
    } catch (e) {
      /* 离线兜底：无 token 时写操作会 403，交由调用方 catch */
    }
    return csrf;
  }

  async function req(path, payload) {
    const ctrl = typeof AbortController !== 'undefined' ? new AbortController() : null;
    const timer = ctrl ? setTimeout(() => ctrl.abort(), 12000) : 0;
    try {
      const headers = { 'content-type': 'application/json' };
      if (payload !== undefined) {
        const tok = await ensureCsrf();
        if (tok) headers['x-csrf-token'] = tok;
      }
      const res = await fetch(API + path, {
        method: payload === undefined ? 'GET' : 'POST',
        credentials: 'include',
        headers,
        body: payload === undefined ? undefined : JSON.stringify(payload),
        signal: ctrl ? ctrl.signal : undefined,
      });
      const body = await res.json().catch(() => null);
      if (!res.ok || !body || body.code !== 0) {
        throw new Error((body && body.message) || ('HTTP ' + res.status));
      }
      return body.data;
    } finally {
      if (timer) clearTimeout(timer);
    }
  }

  async function loadRemote(now) {
    const d = await req('/games/fishing/pond');
    G.__FT_SERVER_MS = d.server_ms || Date.now();
    if (typeof d.spark === 'number') G.__FT_SPARK = d.spark;
    return E.hydrate(d.state, d.server_ms || now || Date.now());
  }

  function castRemote() { return req('/games/fishing/pond/cast', {}); }
  function hookRemote(roundId) { return req('/games/fishing/pond/hook', { round_id: roundId }); }
  function settleRemote(roundId, outcome, durationMs) {
    return req('/games/fishing/pond/settle', { round_id: roundId, outcome, duration_ms: Math.round(durationMs) });
  }
  function actRemote(a, args) { return req('/games/fishing/pond/act', Object.assign({ a }, args || {})); }
  function rankRemote() { return req('/games/fishing/pond/rank'); }

  G.FTStore = {
    load, save, touch, flush, reset, exportText, importText, backend: pickBackend,
    remote: REMOTE, loadRemote, castRemote, hookRemote, settleRemote, actRemote, rankRemote,
  };
})(typeof window !== 'undefined' ? window : globalThis);
