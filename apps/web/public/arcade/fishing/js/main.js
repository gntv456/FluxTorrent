/* 钓鱼玩法 · 主控层
 * 负责：视图路由、事件委托、抛竿→咬钩→溜鱼→结算的状态机、单 rAF 循环、自动垂钓、
 * 存档节流与本地持久化。界面绘制全部交给 ui.js，数值全部交给 engine.js。
 */
(function (G) {
  'use strict';
  const D = G.FTData, E = G.FTEngine, S = G.FTStore, U = G.FTUI;
  // 嵌入站点时由宿主把 FT_ROOT 指到 ShadowRoot，独立运行时就是 document
  const ROOT = G.FT_ROOT || document;
  const $ = (id) => (ROOT.getElementById ? ROOT.getElementById(id) : null);

  let state = S.load(Date.now());
  E.ensureOrders(state, Date.now());
  E.ensureSeason(state, Date.now());
  E.friendState(state, Date.now());

  /** 采纳服务端存档：站点模式下它是唯一事实源，本地只是表现缓存。 */
  function adopt(st, serverMs) {
    if (!st) return;
    state = E.hydrate(st, serverMs || G.__FT_SERVER_MS || Date.now());
    if (typeof st.magic === 'number') G.__FT_SPARK = st.magic;
    E.ensureOrders(state, Date.now());
    E.ensureSeason(state, Date.now());
    E.friendState(state, Date.now());
  }
  function adoptFrom(d) { if (d && d.state) adopt(d.state, G.__FT_SERVER_MS); }

  /** 非抛竿类动作：本地先给即时反馈，随后用服务端结果覆盖（服务端才是权威）。 */
  function syncAction(a, args) {
    if (!S.remote) return;
    S.actRemote(a, args || {}).then((d) => {
      adoptFrom(d);
      if (!d.ok) fail({ reason: d.reason });
      S.touch(state, true);
      paint(rt.view === 'pond');
    }).catch(() => toast('网络异常 · 操作未同步，请重试', 'warn'));
  }

  const rt = {
    view: 'pond',
    shopCat: 'rod',
    phase: 'idle',          // idle | wait | bite | reel
    cast: null,
    bubble: '',
    autoOn: false,
    lastTs: 0,
    raf: 0,
    biteTimer: 0,
    windowTimer: 0,
    autoHook: 0,
    autoTimer: 0,
    hold: false,
    reelRt: null,
    batch: [],
  };
  rt.autoOn = !!state.auto && !!state.autoUnlocked;

  /* ================== 渲染 ================== */
  function updateHud() {
    const d = new Date();
    const t = $('hudTime');
    if (t) t.textContent = String(d.getHours()).padStart(2, '0') + ':' + String(d.getMinutes()).padStart(2, '0');
    const m = $('hudMagic');
    if (m) m.querySelector('b').textContent = U.fmt(state.magic);
    const se = $('hudSeason');
    if (se) {
      const info = E.seasonInfo(state, Date.now());
      se.textContent = `赛季第 ${info.dayIndex + 1} 天 · 剩 ${info.leftDays} 天`;
    }
  }

  function paint(resetScroll) {
    const view = $('view');
    if (!view) return;
    const top = view.scrollTop;
    view.innerHTML = U.render(state, rt.view, rt);
    view.scrollTop = resetScroll ? 0 : top;
    syncTabs();
    updateHud();
    tickOrderLeft();
    const t = $('autoBadge');
    if (t) t.classList.toggle('on', rt.autoOn);
  }

  function syncTabs() {
    ROOT.querySelectorAll('.tabbar .tb').forEach((b) => {
      b.classList.toggle('on', b.dataset.tab === rt.view || (rt.view === 'aquarium' && b.dataset.tab === 'bag'));
    });
  }

  /** 站点模式下的真实赛季榜：拉一次服务端榜单，缓存给 ui 渲染。 */
  let rankLoading = false;
  function loadRank() {
    if (!S.remote || rankLoading) return;
    rankLoading = true;
    S.rankRemote()
      .then((d) => {
        G.__FT_RANK = {
          info: { ordinal: d.season.ordinal, leftDays: d.season.leftDays, dayIndex: d.season.dayIndex },
          season: { catches: d.seasonCatches },
          me: { rank: (d.me && d.me.rank) || 0, catches: d.seasonCatches },
          rows: d.rows || [],
        };
        if (rt.view === 'rank' || rt.view === 'pond') paint(rt.view === 'rank');
      })
      .catch(() => {})
      .finally(() => { rankLoading = false; });
  }

  function setView(v, opts) {
    rt.view = v;
    if (opts && opts.cat) rt.shopCat = opts.cat;
    if (v === 'pond') {
      rt.bubble = rt.phase === 'idle' ? idleBubble() : rt.bubble;
    }
    paint(true);
    if (v === 'rank') loadRank();
  }

  function idleBubble() {
    if (state.stamina.v <= 0 && state.freeCasts <= 0) return '体力耗尽 · 去签到或用魔力补满';
    return `点「抛竿」开始${state.freeCasts ? ' · 免费 ' + state.freeCasts + ' 次' : ''}`;
  }

  function tickOrderLeft() {
    const el = $('orderLeft');
    if (!el) return;
    const now = new Date();
    const mid = new Date(now.getFullYear(), now.getMonth(), now.getDate() + 1, 0, 0, 0);
    const ms = mid - now;
    el.textContent = U.mmss(ms);
  }

  /* ================== 提示 ================== */
  let toastTimer = 0;
  function toast(msg, kind) {
    const wrap = $('toastWrap');
    if (!wrap) return;
    const d = document.createElement('div');
    d.className = 'toast' + (kind ? ' ' + kind : '');
    d.textContent = msg;
    wrap.appendChild(d);
    while (wrap.children.length > 3) wrap.removeChild(wrap.firstChild);
    clearTimeout(toastTimer);
    toastTimer = setTimeout(() => {
      if (d.parentNode) d.parentNode.removeChild(d);
    }, 2400);
  }

  /* ================== 弹层 ================== */
  function openSheet(html) {
    const ov = $('overlay');
    if (!ov || !html) return;
    ov.innerHTML = html;
    ov.classList.add('on');
    ov.setAttribute('aria-hidden', 'false');
  }
  function closeSheet() {
    const ov = $('overlay');
    if (!ov) return;
    ov.classList.remove('on');
    ov.setAttribute('aria-hidden', 'true');
    ov.innerHTML = '';
  }

  /* ================== 抛竿 → 咬钩 ================== */
  function clearCastTimers() {
    clearTimeout(rt.biteTimer); rt.biteTimer = 0;
    clearTimeout(rt.windowTimer); rt.windowTimer = 0;
    clearTimeout(rt.autoHook); rt.autoHook = 0;
  }

  async function castNow() {
    if (rt.phase !== 'idle') return;
    closeSheet();
    // 站点模式：鱼种、咬钩时刻与体力扣减都在服务端定死，客户端不再自己掷骰
    if (S.remote) {
      try {
        const d = await S.castRemote();
        adoptFrom(d);
        if (!d.ok) {
          rt.bubble = '体力不足 · 休息或补满后再抛竿';
          toast('体力不足 · 6 分钟恢复 1 点，或用 ✦40 补满', 'warn');
          if (rt.autoOn) stopAuto('体力不足，自动垂钓已暂停');
          paint();
          return;
        }
        rt.cast = {
          ok: true, fish: d.fish, size: d.size, biteAfter: d.bite_after_ms,
          windowMs: d.window_ms, stats: d.stats, limit: d.limit, pull: d.pull,
          roundId: d.round_id,
        };
        rt.phase = 'wait';
        rt.bubble = '抛竿入水 · 静待鱼讯…';
        S.touch(state);
        if (rt.view === 'pond') paint();
        updateHud();
        rt.biteTimer = setTimeout(onBite, d.bite_after_ms);
        return;
      } catch (e) {
        toast('服务端暂时不可用 · 本竿改用本地存档', 'warn');
      }
    }
    E.recoverStamina(state, Date.now());
    const info = E.cast(state, Math.random, Date.now());
    if (!info.ok) {
      rt.bubble = '体力不足 · 休息或补满后再抛竿';
      toast('体力不足 · 6 分钟恢复 1 点，或用 ✦40 补满', 'warn');
      if (rt.autoOn) stopAuto('体力不足，自动垂钓已暂停');
      paint();
      return;
    }
    rt.cast = info;
    rt.phase = 'wait';
    rt.bubble = '抛竿入水 · 静待鱼讯…';
    S.touch(state);
    if (rt.view === 'pond') paint();
    updateHud();
    rt.biteTimer = setTimeout(onBite, info.biteAfter);
  }

  function remoteResolve(outcome, durationMs) {
    if (!S.remote || !rt.cast || !rt.cast.roundId) return;
    S.settleRemote(rt.cast.roundId, outcome, durationMs || 0).then(adoptFrom).catch(() => {});
  }

  function onBite() {
    if (rt.phase !== 'wait' || !rt.cast) return;
    rt.phase = 'bite';
    rt.bubble = '上钩了！窗口内点「起竿」';
    if (rt.view === 'pond') paint();
    startBiteTick();
    // 窗口结束仍未起竿 → 鱼跑
    rt.windowTimer = setTimeout(() => {
      if (rt.phase !== 'bite') return;
      remoteResolve('escape', 0);
      E.settleMiss(state, 'miss');
      rt.bubble = '起竿太慢 · 鱼跑了';
      toast('错过了起竿窗口 · 换支窗口更长的鱼竿试试', 'warn');
      resetCast();
      S.touch(state);
      if (rt.view === 'pond') paint();
      scheduleAuto();
    }, rt.cast.windowMs);
    // 自动垂钓：窗口前段自动起竿
    if (rt.autoOn) {
      rt.autoHook = setTimeout(() => {
        if (rt.phase === 'bite') hookNow();
      }, Math.max(50, Math.min(170, rt.cast.windowMs * 0.3)));
    }
  }

  let biteRaf = 0;
  function startBiteTick() {
    cancelAnimationFrame(biteRaf);
    const end = performance.now() + rt.cast.windowMs;
    const total = rt.cast.windowMs;
    const step = () => {
      if (rt.phase !== 'bite') return;
      const left = end - performance.now();
      const prog = ROOT.querySelector('#biteProg');
      const hint = ROOT.querySelector('#biteHint');
      if (prog) prog.style.strokeDashoffset = (276.46 * (1 - Math.max(0, left) / total)).toFixed(1);
      if (hint) hint.textContent = '窗口 ' + Math.max(0, Math.round(left)) + 'ms';
      if (left > 0) biteRaf = requestAnimationFrame(step);
    };
    biteRaf = requestAnimationFrame(step);
  }

  async function hookNow() {
    if (rt.phase !== 'bite' || !rt.cast) return;
    cancelAnimationFrame(biteRaf);
    clearTimeout(rt.windowTimer);
    clearTimeout(rt.autoHook);
    if (S.remote && rt.cast.roundId) {
      try {
        const d = await S.hookRemote(rt.cast.roundId);
        if (!d.ok) {
          E.settleMiss(state, 'miss');
          rt.bubble = '起竿太慢 · 鱼跑了';
          toast('错过了起竿窗口 · 换支窗口更长的鱼竿试试', 'warn');
          resetCast();
          S.touch(state);
          if (rt.view === 'pond') paint();
          scheduleAuto();
          return;
        }
      } catch (e) {
        toast('服务端暂不可用 · 本竿按本地判定', 'warn');
      }
    }
    const stats = rt.cast.stats;
    // 中钩瞬间脱钩：基础 10%，被中钩率与脱钩率降低
    const flee = Math.max(0, 0.10 - stats.hookRate - stats.escape * 0.5);
    if (Math.random() < flee) {
      remoteResolve('escape', 0);
      E.settleMiss(state, 'miss');
      rt.bubble = '中钩瞬间脱钩 · 鱼溜走了';
      toast('脱钩了 · 提升中钩率可降低这个概率', 'warn');
      resetCast();
      S.touch(state);
      if (rt.view === 'pond') paint();
      scheduleAuto();
      return;
    }
    startReel();
  }

  function resetCast() {
    rt.phase = 'idle';
    rt.cast = null;
    rt.reelRt = null;
    rt.lastTs = 0;
    rt.hold = false;
    clearCastTimers();
  }

  /* ================== 溜鱼 ================== */
  function startReel() {
    rt.phase = 'reel';
    rt.reelRt = E.makeReelRuntime(rt.cast, rt.cast.fish);
    rt.lastTs = 0;
    rt.hold = false;
    const ov = $('overlay');
    ov.innerHTML = reelStageHtml(rt.cast.fish, rt.cast.stats);
    ov.classList.add('on');
    ov.setAttribute('aria-hidden', 'false');
    bindHold();
    cancelAnimationFrame(rt.raf);
    rt.raf = requestAnimationFrame(reelTick);
  }

  function reelStageHtml(fish, stats) {
    const r = D.RARITY[fish.rarity] || D.RARITY.common;
    const night = state.cond === 'night';
    const t0 = Math.round(rt.reelRt.tension);
    const tPct = Math.max(0, Math.min(100, rt.reelRt.tension / rt.reelRt.limit * 100));
    return `<div class="reel-root ${night ? 'night-a' : 'day-a'}" id="reelRoot">
      <div class="reel-top">
        <span class="rt-tag" style="color:${r.color}">${D.RARITY[fish.rarity].name} · ${U.esc(fish.name)}</span>
        <span style="opacity:.82">${U.esc(D.sceneById[state.scene].name)} · ${U.condName(state.cond)}</span>
      </div>
      <div class="reel-ring">
        <svg viewBox="0 0 160 160">
          <circle class="rr-track" cx="80" cy="80" r="72"/>
          <circle class="rr-prog" id="reelProg" cx="80" cy="80" r="72" stroke-dasharray="452.39" stroke-dashoffset="452.39"/>
        </svg>
        <div class="rr-inner">
          <svg class="ic"><use href="#i-fish"/></svg>
          <span class="rr-txt" id="reelPct">0%</span>
          <span class="rr-sub">收线进度</span>
        </div>
      </div>
      <div class="tension" id="tensionBox">
        <div class="tn-label"><span>线张力</span><span id="tnVal">${t0} / ${Math.round(stats.limit)}</span></div>
        <div class="tn-bar">
          <div class="tn-safe" style="width:${Math.round(rt.reelRt.softLimit / rt.reelRt.limit * 100)}%"></div>
          <div class="tn-fill" id="tnFill" style="width:${tPct.toFixed(1)}%"></div>
          <div class="tn-needle" id="tnNeedle" style="left:${tPct.toFixed(1)}%"></div>
        </div>
      </div>
      <div class="reel-tip">按住「收线」拉近距离，张力进红区前松手泄力；张力触顶即断线跑鱼。</div>
      <button class="reel-hold" id="holdBtn">按住收线</button>
      <div class="reel-timer" id="reelTimer">剩 ${rt.reelRt.maxTime.toFixed(1)}s</div>
    </div>`;
  }

  function bindHold() {
    const btn = $('holdBtn');
    if (!btn) return;
    const on = (e) => {
      e.preventDefault();
      rt.hold = true;
      btn.classList.add('on');
      if (rt.reelRt) rt.reelRt.holding = true;
    };
    const off = () => {
      rt.hold = false;
      if (btn) btn.classList.remove('on');
      if (rt.reelRt) rt.reelRt.holding = false;
    };
    btn.addEventListener('pointerdown', on);
    btn.addEventListener('pointerup', off);
    btn.addEventListener('pointerleave', off);
    btn.addEventListener('pointercancel', off);
    btn.addEventListener('contextmenu', (e) => e.preventDefault());
    rt.holdOff = off;
    rt.holdOn = on;
  }

  function reelTick(ts) {
    if (rt.phase !== 'reel' || !rt.reelRt) return;
    const dt = Math.min(0.05, Math.max(0.001, (ts - (rt.lastTs || ts - 16)) / 1000));
    rt.lastTs = ts;
    if (rt.autoOn) rt.reelRt.holding = rt.reelRt.tension < rt.reelRt.softLimit * 0.82;
    const res = E.reelStep(rt.reelRt, dt);
    applyReelUi();
    if (res.broken) { endReel('break'); return; }
    if (res.landed) { endReel('land'); return; }
    if (res.escaped) { endReel('escape'); return; }
    rt.raf = requestAnimationFrame(reelTick);
  }

  function applyReelUi() {
    const p = rt.reelRt;
    if (!p) return;
    const prog = $('reelProg'), pct = $('reelPct'), fill = $('tnFill'), needle = $('tnNeedle'),
      val = $('tnVal'), timer = $('reelTimer'), box = $('tensionBox'), hold = $('holdBtn');
    if (prog) prog.style.strokeDashoffset = (452.39 * (1 - p.progress / 100)).toFixed(1);
    if (pct) pct.textContent = Math.round(p.progress) + '%';
    const tp = Math.max(0, Math.min(100, p.tension / p.limit * 100));
    if (fill) fill.style.width = tp.toFixed(1) + '%';
    if (needle) needle.style.left = tp.toFixed(1) + '%';
    if (val) val.textContent = Math.round(p.tension) + ' / ' + Math.round(p.limit);
    if (timer) timer.textContent = '剩 ' + Math.max(0, p.maxTime - p.elapsed).toFixed(1) + 's';
    if (box) box.classList.toggle('hot', tp > 86);
    if (hold) hold.classList.toggle('on', !!p.holding);
  }

  function endReel(kind) {
    cancelAnimationFrame(rt.raf);
    rt.raf = 0;
    if (rt.holdOff) rt.holdOff();
    const ov = $('overlay');
    if (ov) { ov.classList.remove('on'); ov.innerHTML = ''; ov.setAttribute('aria-hidden', 'true'); }

    if (kind === 'break') {
      remoteResolve('break', rt.reelRt ? rt.reelRt.elapsed * 1000 : 0);
      E.settleMiss(state, 'break');
      rt.bubble = '张力触顶 · 断线跑鱼';
      toast('断线了 · 换更强的鱼线或放慢收线节奏', 'warn');
      resetCast();
      S.touch(state, true);
      if (rt.view === 'pond') paint();
      scheduleAuto();
      return;
    }
    if (kind === 'escape') {
      remoteResolve('escape', rt.reelRt ? rt.reelRt.elapsed * 1000 : 0);
      E.settleMiss(state, 'miss');
      rt.bubble = '溜鱼超时 · 鱼脱钩了';
      toast('溜鱼超时 · 用收线效率更高的鱼竿缩短时间', 'warn');
      resetCast();
      S.touch(state, true);
      if (rt.view === 'pond') paint();
      scheduleAuto();
      return;
    }
    settle();
  }

  /* ================== 结算 ================== */
  async function settle() {
    const cast = rt.cast;
    // 站点模式：结算发奖、图鉴/订单/赛季全部由服务端写入，这里只负责展示
    if (S.remote && cast && cast.roundId) {
      try {
        const dur = rt.reelRt ? rt.reelRt.elapsed * 1000 : 0;
        const d = await S.settleRemote(cast.roundId, 'land', dur);
        adoptFrom(d);
        if (!d.ok) {
          toast('结算失败，请重试', 'warn');
          resetCast();
          if (rt.view === 'pond') paint();
          scheduleAuto();
          return;
        }
        const items = d.items || [];
        resetCast();
        S.touch(state, true);
        if (items.length) {
          rt.bubble = `上鱼了 · ${items[0].name} ✦${items[0].magic}`;
        }
        if (rt.view === 'pond') paint();
        if (rt.autoOn) {
          rt.batch.push(...items);
          const sum = items.reduce((a, x) => a + x.magic, 0);
          toast(`+${items.length} 尾 · ✦${sum}`, 'ok');
          if (rt.batch.length >= 6) {
            openSheet(U.catchSheet(state, { items: rt.batch.slice() }));
            rt.batch = [];
            rt.autoHold = true;
          } else {
            scheduleAuto(900);
          }
          return;
        }
        openSheet(U.catchSheet(state, { items }));
        return;
      } catch (e) {
        toast('服务端暂不可用 · 本竿按本地结算', 'warn');
      }
    }
    const stats = cast.stats;
    const items = [];
    const first = E.settleCatch(state, cast.fish, cast, Math.random);
    items.push({ ...first, id: cast.fish.id, name: cast.fish.name, rarity: cast.fish.rarity });

    // 连竿：传说套 / 高等级鱼竿触发额外 1–2 尾
    let extra = 0;
    if (stats.setId === 'dragonLegend') extra = Math.random() < 0.34 ? (Math.random() < 0.28 ? 2 : 1) : 0;
    else if ((E.itemLv(state, 'rod', state.equipped.rod) || 1) >= 4) extra = Math.random() < 0.16 ? 1 : 0;
    for (let i = 0; i < extra; i++) {
      const f = E.rollFish(state, Math.random);
      const res = E.settleCatch(state, f, cast, Math.random);
      items.push({ ...res, id: f.id, name: f.name, rarity: f.rarity });
    }

    E.addSeasonCatch(state, Date.now());
    resetCast();
    S.touch(state, true);
    const sum = items.reduce((a, x) => a + x.magic, 0);
    rt.bubble = `上鱼了 · ${items[0].name} ✦${items[0].magic}`;
    if (rt.view === 'pond') paint();

    if (rt.autoOn) {
      rt.batch.push(...items);
      toast(`+${items.length} 尾 · ✦${sum}`, 'ok');
      if (rt.batch.length >= 6) {
        openSheet(U.catchSheet(state, { items: rt.batch.slice() }));
        rt.batch = [];
        rt.autoHold = true;    // 等玩家关掉结算页再继续
      } else {
        scheduleAuto(900);
      }
      return;
    }
    openSheet(U.catchSheet(state, { items }));
  }

  /* ================== 自动垂钓 ================== */
  function scheduleAuto(delay) {
    if (!rt.autoOn || rt.autoHold) return;
    clearTimeout(rt.autoTimer);
    rt.autoTimer = setTimeout(() => {
      if (!rt.autoOn || rt.autoHold) return;
      if (E.bagFree(state) <= 0 && state.aquarium.length >= E.aquariumCap(state)) {
        stopAuto('背包与临时鱼缸都已满，自动垂钓已停止');
        return;
      }
      castNow();
    }, delay || 800);
  }

  function stopAuto(msg) {
    rt.autoOn = false;
    rt.autoHold = false;
    state.auto = false;
    clearTimeout(rt.autoTimer); rt.autoTimer = 0;
    clearTimeout(rt.autoHook); rt.autoHook = 0;
    if (msg) toast(msg, 'warn');
    S.touch(state);
    if (rt.view === 'pond') paint();
  }

  function toggleAuto() {
    if (!state.autoUnlocked) {
      toast('自动垂钓未解锁 · 商店 ✦' + D.CONST.autoUnlockPrice + ' 一次买断', 'warn');
      setView('shop', { cat: 'auto' });
      return;
    }
    if (rt.autoOn) {
      rt.autoOn = false;
      rt.autoHold = false;
      state.auto = false;
      clearTimeout(rt.autoTimer); rt.autoTimer = 0;
      clearTimeout(rt.autoHook); rt.autoHook = 0;
      toast('自动垂钓已关闭'); syncAction('setAuto', { on: false });
      S.touch(state);
      paint();
      return;
    }
    rt.autoOn = true;
    rt.autoHold = false;
    state.auto = true;
    toast('自动垂钓已开启 · 仍会消耗体力', 'ok'); syncAction('setAuto', { on: true });
    S.touch(state);
    if (rt.phase === 'idle') scheduleAuto(600);
    else if (rt.phase === 'bite') hookNow();
    paint();
  }

  /* ================== 业务动作 ================== */
  const act = {
    cast() { castNow(); },
    hook() { hookNow(); },
    toggleAuto() { toggleAuto(); },
    closeSheet() {
      closeSheet();
      if (rt.autoHold) { rt.autoHold = false; scheduleAuto(700); }
    },
    openView(el) {
      const v = el.dataset.view;
      if (v === 'shop' && el.dataset.cat) setView('shop', { cat: el.dataset.cat });
      else setView(v);
    },
    gotoPond() { closeSheet(); setView('pond'); },
    shopCat(el) { rt.shopCat = el.dataset.cat; paint(true); },
    openItem(el) { openSheet(U.itemSheet(state, el.dataset.cat, el.dataset.id)); },
    buy(el) {
      const cat = el.dataset.cat, id = el.dataset.id;
      const r = E.buyItem(state, cat, id);
      if (!r.ok) return fail(r);
      E.equipItem(state, cat, id);
      toast(`已购买并装备 ${D.itemById[id].name}`, 'ok'); syncAction('buy', { cat, id });
      closeSheet(); S.touch(state); paint();
    },
    equip(el) {
      const r = E.equipItem(state, el.dataset.cat, el.dataset.id);
      if (!r.ok) return fail(r);
      toast('已装备 · 属性立即生效', 'ok'); syncAction('equip', { cat: el.dataset.cat, id: el.dataset.id });
      closeSheet(); S.touch(state); paint();
    },
    grow(el) { openSheet(U.growSheet(state, el.dataset.cat, el.dataset.id)); },
    upgrade(el) {
      const r = E.upgradeItem(state, el.dataset.cat, el.dataset.id);
      if (!r.ok) return fail(r);
      toast(`升级成功 · Lv.${r.lv} · ✦${r.cost}`, 'ok'); syncAction('upgrade', { cat: el.dataset.cat, id: el.dataset.id });
      openSheet(U.growSheet(state, el.dataset.cat, el.dataset.id));
      S.touch(state); paint();
    },
    buyScene(el) {
      const r = E.buyScene(state, el.dataset.id);
      if (!r.ok) return fail(r);
      toast('已解锁 ' + D.sceneById[el.dataset.id].name, 'ok'); syncAction('buyScene', { id: el.dataset.id });
      S.touch(state); paint();
    },
    switchScene(el) {
      const r = E.switchScene(state, el.dataset.id);
      if (!r.ok) return fail(r);
      toast('已进入 ' + D.sceneById[el.dataset.id].name, 'ok'); syncAction('switchScene', { id: el.dataset.id });
      S.touch(state);
      if (rt.view === 'shop') paint(); else setView('pond');
    },
    openSceneSheet() { openSheet(U.sceneSheet(state)); },
    cond(el) {
      const r = E.setCond(state, el.dataset.c);
      if (!r.ok) return fail(r);
      closeSheet();
      toast('已切换到' + U.condName(el.dataset.c), 'ok'); syncAction('cond', { c: el.dataset.c });
      S.touch(state);
      if (rt.view === 'pond') paint(); else setView('pond');
    },
    gotoScene(el) {
      const id = el.dataset.id, cond = el.dataset.cond;
      if (state.scenesOwned.indexOf(id) < 0) {
        toast('该钓场尚未解锁 · 商店 ✦' + D.sceneById[id].price, 'warn');
        setView('shop', { cat: 'scene' });
        return;
      }
      E.switchScene(state, id);
      E.setCond(state, cond);
      closeSheet();
      toast('已前往 ' + D.sceneById[id].name + ' · ' + U.condName(cond), 'ok'); syncAction('gotoScene', { id, cond });
      S.touch(state); setView('pond');
    },
    expandBag() {
      const r = E.expandBag(state);
      if (!r.ok) return fail(r);
      toast(`背包已扩容至 ${r.cap} 格 · ✦${r.cost}`, 'ok'); syncAction('expandBag');
      S.touch(state); paint();
    },
    unlockAuto() {
      const r = E.unlockAuto(state);
      if (!r.ok) return fail(r);
      toast('自动垂钓已解锁', 'ok'); syncAction('unlockAuto');
      S.touch(state); paint();
    },
    claimOrder(el) {
      const r = E.claimOrder(state, Number(el.dataset.idx));
      if (!r.ok) return fail(r);
      toast(`订单奖励到账 ✦${r.reward}`, 'ok'); syncAction('claimOrder', { idx: Number(el.dataset.idx) });
      S.touch(state); paint();
    },
    claimOrderBonus() {
      const r = E.claimOrderBonus(state);
      if (!r.ok) return fail(r);
      toast(`全部订单完成 · 额外 ✦${r.reward}`, 'ok'); syncAction('claimOrderBonus');
      S.touch(state); paint();
    },
    sign() {
      const r = E.claimSign(state, Date.now());
      if (!r.ok) return fail(r);
      toast('签到成功 · ' + r.reward.label, 'ok'); syncAction('sign');
      S.touch(state); paint();
    },
    refill() {
      const r = E.refillStamina(state, Date.now());
      if (!r.ok) return fail(r);
      toast('体力已补满 · ✦' + r.cost, 'ok'); syncAction('refill');
      S.touch(state); paint();
    },
    gift(el) {
      const r = E.giftBait(state, el.dataset.id, Date.now());
      if (!r.ok) return fail(r);
      toast(`已赠送鱼饵 · 今日还剩 ${r.left} 次`, 'ok'); syncAction('gift', { id: el.dataset.id });
      S.touch(state);
      if (rt.view === 'friends') paint(); else closeSheet();
    },
    receiveGifts() {
      const r = E.receiveGifts(state, Date.now());
      if (!r.ok) return fail(r);
      toast(`收到 ${r.count} 份鱼饵 · 已转为免费抛竿`, 'ok'); syncAction('receiveGifts');
      S.touch(state); paint();
    },
    visitFriend(el) { openSheet(U.friendSheet(state, el.dataset.id)); },
    claimAch(el) {
      const r = E.claimAch(state, el.dataset.id);
      if (!r.ok) return fail(r);
      toast(`成就达成 · ✦${r.reward}`, 'ok'); syncAction('claimAch', { id: el.dataset.id });
      S.touch(state); paint();
    },
    openFish(el) {
      openSheet(U.fishSheet(state, { id: el.dataset.id, uid: el.dataset.uid }));
    },
    releaseBagFish(el) {
      const r = E.releaseFromBag(state, el.dataset.uid);
      if (!r.ok) return fail(r);
      toast('已放生 · 腾出 1 格容量', 'ok'); syncAction('releaseBagFish', { uid: el.dataset.uid });
      closeSheet(); S.touch(state, true); paint();
    },
    takeFish(el) {
      const r = E.takeFromAquarium(state, el.dataset.uid);
      if (!r.ok) return fail(r);
      toast('已取回背包', 'ok'); syncAction('takeFish', { uid: el.dataset.uid });
      S.touch(state, true); paint();
    },
    releaseFish(el) {
      const r = E.releaseFromAquarium(state, el.dataset.uid);
      if (!r.ok) return fail(r);
      toast('已放生', 'ok'); syncAction('releaseFish', { uid: el.dataset.uid });
      S.touch(state, true); paint();
    },
    takeAll() {
      const r = E.takeAllFromAquarium(state);
      if (!r.ok) return fail(r);
      toast(`已取回 ${r.moved} 尾${r.left ? ' · 仍有 ' + r.left + ' 尾（背包已满）' : ''}`, 'ok'); syncAction('takeAll');
      S.touch(state, true); paint();
    },
    releaseAll() {
      if (!state.aquarium.length) return;
      const n = state.aquarium.length;
      state.aquarium = [];
      toast(`已放生 ${n} 尾`, 'ok'); syncAction('releaseAll');
      S.touch(state, true); paint();
    },
    equipSet(el) {
      const id = el.dataset.id;
      const s = D.setById[id];
      const prog = E.setProgress(state).find((p) => p.set.id === id);
      if (!prog || prog.got < prog.total) return toast('该套装还未集齐', 'warn');
      if (state.scene !== s.scene) {
        const r = E.switchScene(state, s.scene);
        if (!r.ok) return fail(r);
      }
      Object.keys(s.parts).forEach((cat) => E.equipItem(state, cat, s.parts[cat]));
      toast('套装已装配 · ' + s.name, 'ok'); syncAction('equipSet', { id });
      S.touch(state); paint();
    },
    openSetParts(el) { openSheet(U.setPartsSheet(state, el.dataset.id)); },
    rules() { openSheet(U.rulesSheet(state)); },
    exportSave() {
      const text = S.exportText(state);
      try {
        const a = document.createElement('a');
        a.href = 'data:application/json;charset=utf-8,' + encodeURIComponent(text);
        a.download = 'fishing-save-' + E.todayKey(Date.now()) + '.json';
        document.body.appendChild(a); a.click(); document.body.removeChild(a);
        toast('存档已导出', 'ok');
      } catch (e) { toast('导出失败 · 可复制控制台输出', 'warn'); }
    },
    resetConfirm() {
      openSheet(U.confirmSheet({
        title: '重置钓鱼进度',
        sub: '将清空魔力、装备、渔获、图鉴、订单与成就进度，此操作不可撤销。',
        okText: '确认重置',
        okAct: 'doReset',
        danger: true,
      }));
    },
    doReset() {
      state = S.reset(Date.now());
      E.ensureOrders(state, Date.now());
      E.ensureSeason(state, Date.now());
      E.friendState(state, Date.now());
      rt.batch = []; rt.autoOn = false; rt.autoHold = false;
      closeSheet(); resetCast();
      rt.bubble = idleBubble();
      setView('pond');
      toast('进度已重置', 'ok'); syncAction('reset');
    },
    tab(el) { setView(el.dataset.tab); },
    noop() {},
  };

  function fail(r) {
    const map = {
      poor: '站点魔力不足', owned: '已经拥有', notowned: '尚未拥有', max: '已是最高档',
      locked: '尚未解锁前置', full: '背包已满', limit: '今日次数已用完', claimed: '已经领取过',
      progress: '进度未完成', empty: '暂无可收', noStamina: '体力不足', missing: '目标已不存在',
    };
    toast(map[r.reason] || '操作不可用', 'warn');
  }

  /* ================== 事件 ================== */
  function onClick(e) {
    // 咬钩窗口内点水面也可起竿
    if (rt.phase === 'bite' && e.target.closest('#water') && !e.target.closest('[data-act]')) {
      hookNow();
      return;
    }
    const el = e.target.closest('[data-act]');
    if (!el) return;
    const name = el.dataset.act;
    if (name && act[name]) { act[name](el, e); }
  }

  function onKey(e) {
    if (e.code === 'Space') {
      if (rt.phase === 'reel' && rt.reelRt) {
        e.preventDefault();
        rt.reelRt.holding = true;
        const b = $('holdBtn'); if (b) b.classList.add('on');
      } else if (rt.phase === 'bite') {
        e.preventDefault();
        hookNow();
      }
    }
    if (e.code === 'Escape') closeSheet();
  }
  function onKeyUp(e) {
    if (e.code === 'Space' && rt.phase === 'reel' && rt.reelRt) {
      rt.reelRt.holding = false;
      const b = $('holdBtn'); if (b) b.classList.remove('on');
    }
  }

  /* ================== 预览钩子（仅用于验收 / 截图，不参与正常玩法） ================== */
  function preview(kind) {
    const fakeCast = () => {
      E.recoverStamina(state, Date.now());
      const s = E.statsOf(state);
      const fish = E.rollFish(state, Math.random);
      rt.cast = { ok: true, fish, biteAfter: 3000, windowMs: s.window, stats: s, limit: s.limit, pull: s.pull, surface: E.computeSurface(state) };
      return s;
    };
    if (kind === 'bite') {
      fakeCast();
      rt.phase = 'bite';
      rt.bubble = '上钩了！窗口内点「起竿」';
      paint(); startBiteTick();
    } else if (kind === 'reel') {
      fakeCast();
      rt.phase = 'reel';
      startReel();
    } else if (kind === 'catch' || kind === 'batch') {
      const f1 = D.fishById['moonfish'] || D.FISH[0];
      const f2 = D.fishById['aurora'] || D.FISH[1];
      const f3 = D.fishById['coralDragon'] || D.FISH[2];
      const mk = (f, magic, size, isNew, dest) => ({ id: f.id, name: f.name, rarity: f.rarity, magic, size, weight: +(size * 0.03).toFixed(2), dest, isNew, albumCount: 7 });
      const items = kind === 'batch'
        ? [mk(f1, 372, 32.4, true, 'bag'), mk(f2, 330, 41.2, false, 'bag'), mk(f3, 920, 78.5, true, 'aquarium')]
        : [mk(f1, 372, 32.4, true, 'bag')];
      openSheet(U.catchSheet(state, { items }));
    } else if (kind === 'sheet') {
      openSheet(U.itemSheet(state, 'rod', 'dragon'));
    } else if (kind === 'grow') {
      openSheet(U.growSheet(state, 'rod', 'moon'));
    } else if (kind === 'scene') {
      openSheet(U.sceneSheet(state));
    } else if (kind === 'rules') {
      openSheet(U.rulesSheet(state));
    } else if (kind === 'friend') {
      openSheet(U.friendSheet(state, D.FRIENDS[0].id));
    } else if (kind === 'setparts') {
      openSheet(U.setPartsSheet(state, 'moonEpic'));
    }
  }

  /* ================== 浏览器内自检（?selftest=1，仅验收用） ================== */
  function runSelfTest() {
    const out = [];
    const ok = (cond, name) => out.push((cond ? 'PASS ' : 'FAIL ') + name);
    const realRandom = Math.random;
    try {
      Math.random = () => 0.99;           // 固定随机：确保不脱钩、不连竿，结果可复现
      rt.autoOn = false; rt.autoHold = false;
      closeSheet(); setView('pond');
      state.stamina = { v: D.CONST.staminaMax, at: Date.now() };
      state.freeCasts = 0;
      S.touch(state);

      const stamBefore = state.stamina.v;
      castNow();
      ok(rt.phase === 'wait', '抛竿 → 等待阶段');
      ok(state.stamina.v === stamBefore - 1, '抛竿扣 1 点体力');
      ok(!!ROOT.querySelector('.pond-cta [data-act="cast"][disabled]'), '等待期主按钮禁用');
      ok(!!ROOT.querySelector('#pondBubble'), '水面气泡存在');

      onBite();
      ok(rt.phase === 'bite', '咬钩 → 起竿窗口');
      ok(!!ROOT.querySelector('#bitePanel.on'), '起竿面板亮起');
      ok(!!ROOT.querySelector('.pond-cta [data-act="hook"]'), '主按钮切换为「起竿」');

      hookNow();
      ok(rt.phase === 'reel', '起竿 → 溜鱼');
      ok(!!ROOT.querySelector('#reelRoot'), '溜鱼全屏层已挂载');

      cancelAnimationFrame(rt.raf); rt.raf = 0;
      let res = null;
      for (let i = 0; i < 4000; i++) {
        rt.reelRt.holding = rt.reelRt.tension < rt.reelRt.softLimit * 0.9;
        res = E.reelStep(rt.reelRt, 0.016);
        if (res.landed || res.broken) break;
      }
      ok(!!res && !res.broken, '按节奏收线不会断线');
      ok(!!res && res.landed, '按住收线可拉满进度');

      const albumBefore = Object.keys(state.album).length;
      const bagBefore = state.bag.fish.length;
      endReel('land');
      ok(!!ROOT.querySelector('#sheetBody'), '结算弹层已打开');
      ok(state.bag.fish.length + state.aquarium.length > bagBefore, '渔获已入包或入缸');
      ok(Object.keys(state.album).length >= albumBefore, '图鉴已登记');

      closeSheet();
      setView('pond');
      ok(rt.phase === 'idle', '结算后回到待机');
    } catch (err) {
      out.push('FAIL 异常：' + (err && err.message));
    } finally {
      Math.random = realRandom;
    }
    const dbg = $('dbg');
    if (dbg) { dbg.style.display = 'block'; dbg.textContent = out.join('\n'); }
    const allPass = !out.some((l) => l.indexOf('FAIL') === 0);
    document.title = 'SELFTEST ' + (allPass ? 'PASS' : 'FAIL');
    return out;
  }

  /* ================== 启动 ================== */
  let hbTimer = 0;
  let debTimer = 0;
  const onVisibility = () => {
    if (document.hidden) { S.flush(state); }
    else {
      const g = E.recoverStamina(state, Date.now());
      if (g > 0) { S.touch(state); paint(); }
    }
  };
  const onPageHide = () => S.flush(state);
  const onResize = () => {
    clearTimeout(debTimer);
    debTimer = setTimeout(() => { if (rt.view === 'pond') paint(); }, 180);
  };

  async function boot() {
    // 站点模式：先把云端存档拉下来，再画第一帧，避免看到本地旧进度（或别人账号的缓存）
    if (S.remote) {
      try {
        adopt(await S.loadRemote());
      } catch (e) {
        toast('云存档加载失败 · 先用本地缓存顶上', 'warn');
      }
    }
    paint(true);
    if (S.remote) loadRank(); // 侧栏“赛季排行”显示真实名次，而不是 NPC 模拟值
    ROOT.addEventListener('click', onClick);
    ROOT.addEventListener('keydown', onKey);
    ROOT.addEventListener('keyup', onKeyUp);

    // 全局 1 秒心跳：时钟、订单倒计时、体力自然恢复
    let acc = 0;
    hbTimer = setInterval(() => {
      updateHud();
      tickOrderLeft();
      acc += 1;
      if (acc % 20 === 0) {
        const gained = E.recoverStamina(state, Date.now());
        if (gained > 0) {
          S.touch(state);
          if (rt.view === 'pond' || rt.view === 'stamina') paint();
        }
        // 若当前体力耗尽且自动垂钓开着，暂停
        if (rt.autoOn && state.stamina.v <= 0 && state.freeCasts <= 0) stopAuto('体力耗尽，自动垂钓已暂停');
      }
      if (rt.phase === 'idle') {
        const b = $('pondBubble');
        if (b && rt.view === 'pond' && !rt.autoOn) {
          const want = idleBubble();
          if (b.textContent !== want) b.textContent = want;
        }
      }
    }, 1000);

    document.addEventListener('visibilitychange', onVisibility);
    window.addEventListener('pagehide', onPageHide);
    window.addEventListener('resize', onResize);

    rt.bubble = idleBubble();
    // 便于验收/截图：index.html?view=shop&cat=bag 可直接落到对应页面
    try {
      const q = new URLSearchParams(location.search);
      const v = q.get('view');
      if (v) { rt.view = v; if (q.get('cat')) rt.shopCat = q.get('cat'); }
      const pv = q.get('preview');
      if (pv) { rt.view = 'pond'; setTimeout(() => preview(pv), 200); }
      if (q.get('selftest')) setTimeout(runSelfTest, 420);
    } catch (e) { /* file:// 下无 query 也正常 */ }
    paint(true);
    if (rt.autoOn) scheduleAuto(1200);
    if (location.search.indexOf('debug') >= 0) {
      const fr = ROOT.querySelector('.frame'), vw = $('view'), qg = ROOT.querySelector('.quick-grid'), gs = ROOT.querySelector('.gear-strip'), tb = $('tabbar');
      const dbg = $('dbg');
      if (dbg) {
        dbg.style.display = 'block';
        dbg.textContent =
          `inner=${G.innerWidth}x${G.innerHeight} dpr=${G.devicePixelRatio}\n` +
          `frame=${fr && Math.round(fr.getBoundingClientRect().width)} viewScrollW=${vw && vw.scrollWidth} clientW=${vw && vw.clientWidth}\n` +
          `quickGrid=${qg && Math.round(qg.getBoundingClientRect().width)} scrollW=${qg && qg.scrollWidth}\n` +
          `gearStrip=${gs && Math.round(gs.getBoundingClientRect().width)} scrollW=${gs && gs.scrollWidth}\n` +
          `tabbar=${tb && Math.round(tb.getBoundingClientRect().width)} docScrollW=${document.documentElement.scrollWidth}`;
      }
    }
    console.log('[fishing] ready · backend=' + S.backend() + ' · fish=' + D.FISH.length);
  }

  function debounce(fn, ms) {
    let t = 0;
    return function () { clearTimeout(t); t = setTimeout(fn, ms); };
  }

  /** 宿主卸载时调用：停掉心跳/计时器/rAF 与所有监听，避免多实例残留 */
  function destroy() {
    try { S.flush(state); } catch (e) { /* 忽略销毁期存档异常 */ }
    clearInterval(hbTimer); hbTimer = 0;
    clearTimeout(debTimer); debTimer = 0;
    clearTimeout(rt.biteTimer); clearTimeout(rt.windowTimer);
    clearTimeout(rt.autoHook); clearTimeout(rt.autoTimer);
    rt.biteTimer = rt.windowTimer = rt.autoHook = rt.autoTimer = 0;
    cancelAnimationFrame(rt.raf); rt.raf = 0;
    cancelAnimationFrame(biteRaf);
    if (rt.holdOff) { try { rt.holdOff(); } catch (e) { /* 已脱离文档 */ } }
    ROOT.removeEventListener('click', onClick);
    ROOT.removeEventListener('keydown', onKey);
    ROOT.removeEventListener('keyup', onKeyUp);
    document.removeEventListener('visibilitychange', onVisibility);
    window.removeEventListener('pagehide', onPageHide);
    window.removeEventListener('resize', onResize);
    rt.phase = 'idle';
  }

  if (document.readyState === 'loading') document.addEventListener('DOMContentLoaded', boot);
  else boot();

  G.FTApp = { get state() { return state; }, rt, destroy };
  G.__FT_FISHING_DESTROY__ = destroy;
})(typeof window !== 'undefined' ? window : globalThis);
