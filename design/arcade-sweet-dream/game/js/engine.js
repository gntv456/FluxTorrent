/* 钓鱼玩法 · 纯逻辑引擎
 * 不依赖 DOM / localStorage；浏览器挂 window.FTEngine，Node 挂 globalThis.FTEngine。
 * 所有函数接收 state 并就地读写，返回结果对象，便于单测与后续接 Rust API 对齐口径。
 */
(function (G) {
  'use strict';
  const D = G.FTData;
  const C = D.CONST;

  /* ================= 基础工具 ================= */

  function mulberry32(seed) {
    let a = seed >>> 0;
    return function () {
      a = (a + 0x6d2b79f5) >>> 0;
      let t = a;
      t = Math.imul(t ^ (t >>> 15), t | 1);
      t ^= t + Math.imul(t ^ (t >>> 7), t | 61);
      return ((t ^ (t >>> 14)) >>> 0) / 4294967296;
    };
  }

  const DAY_MS = 86400000;
  function todayKey(now) {
    const d = new Date(now == null ? Date.now() : now);
    const m = String(d.getMonth() + 1).padStart(2, '0');
    const day = String(d.getDate()).padStart(2, '0');
    return `${d.getFullYear()}-${m}-${day}`;
  }
  function dayNumber(now) { return Math.floor((now == null ? Date.now() : now) / DAY_MS); }

  function clamp(v, a, b) { return v < a ? a : v > b ? b : v; }
  function round2(v) { return Math.round(v * 100) / 100; }

  /* ================= 体力 ================= */

  /** 结算离线体力：按 staminaRecoverMs 递增，满则停止累积。 */
  function recoverStamina(state, now) {
    const t = now == null ? Date.now() : now;
    const st = state.stamina;
    if (st.v >= C.staminaMax) { st.at = t; return 0; }
    if (!st.at || st.at > t) { st.at = t; return 0; }
    const gained = Math.floor((t - st.at) / C.staminaRecoverMs);
    if (gained <= 0) return 0;
    const before = st.v;
    st.v = Math.min(C.staminaMax, st.v + gained);
    st.at = st.v >= C.staminaMax ? t : st.at + gained * C.staminaRecoverMs;
    return st.v - before;
  }

  /** 距下一点体力恢复的毫秒数（满体力返回 0）。 */
  function staminaNextIn(state, now) {
    const t = now == null ? Date.now() : now;
    if (state.stamina.v >= C.staminaMax) return 0;
    return Math.max(0, state.stamina.at + C.staminaRecoverMs - t);
  }

  function refillStamina(state, now) {
    const need = C.staminaMax - state.stamina.v;
    if (need <= 0) return { ok: false, reason: 'full' };
    if (state.magic < C.staminaRefillCost) return { ok: false, reason: 'poor' };
    state.magic -= C.staminaRefillCost;
    state.stamina.v = C.staminaMax;
    state.stamina.at = now == null ? Date.now() : now;
    return { ok: true, cost: C.staminaRefillCost };
  }

  /* ================= 装备与属性汇总 ================= */

  function ownedEntry(state, cat, id) {
    return (state.owned[cat] || []).find((x) => x.id === id) || null;
  }
  function isOwned(state, cat, id) { return !!ownedEntry(state, cat, id); }
  function itemLv(state, cat, id) {
    const e = ownedEntry(state, cat, id);
    return e ? e.lv || 1 : 0;
  }

  /** 汇总全部生效属性；养成等级按 +8%/级 叠加到主要字段。 */
  function statsOf(state) {
    const s = {
      window: 800, biteSpeed: 0, hookRate: 0, escape: 0, rare: 0, legend: 0,
      limit: 100, alert: 0, pull: 1, bigFish: 0, night: 0, common: 0,
      all: 0, setId: null, setBonus: [],
    };
    ['rod', 'bait', 'hook', 'line', 'float'].forEach((cat) => {
      const id = state.equipped[cat];
      const it = D.itemById[id];
      if (!it) return;
      const lv = Math.max(1, itemLv(state, cat, id) || 1);
      const lvBonus = (lv - 1) / (C.upgradeMaxLv - 1); // 0..1
      const e = it.eff || {};
      if (e.window) s.window += e.window * (1 + lvBonus * 0.35);
      if (e.biteSpeed) s.biteSpeed += e.biteSpeed * (1 + lvBonus * 0.3);
      if (e.hookRate) s.hookRate += e.hookRate * (1 + lvBonus * 0.3);
      if (e.escape) s.escape += e.escape * (1 + lvBonus * 0.3);
      if (e.rare) s.rare += e.rare * (1 + lvBonus * 0.3);
      if (e.legend) s.legend += e.legend * (1 + lvBonus * 0.3);
      if (e.limit) s.limit += e.limit * (1 + lvBonus * 0.4);
      if (e.alert) s.alert += e.alert * (1 + lvBonus * 0.3);
      if (e.pull) s.pull += e.pull * (1 + lvBonus * 0.3);
      if (e.bigFish) s.bigFish += e.bigFish * (1 + lvBonus * 0.2);
      if (e.night) s.night += e.night * (1 + lvBonus * 0.2);
    });
    const set = activeSet(state);
    if (set) {
      s.setId = set.id;
      s.setBonus = set.bonuses.slice();
      set.bonuses.forEach((b) => {
        if (b.k === 'all') { s.all += b.v; }
        else if (b.k === 'nightRare') { s.rare += b.v; }
        else if (b.k === 'deepFish') { s.rare += b.v; }
        else if (s[b.k] != null) s[b.k] += b.v;
        else s[b.k] = b.v;
      });
    }
    // alert 降低鱼群警觉 → 窗口延长；bigFish 降低拉力上升
    s.window = Math.round(s.window * (1 + s.alert + s.all));
    s.pull = round2(s.pull + s.all);
    s.limit = round2(s.limit + s.all * 40);
    s.biteSpeed = round2(s.biteSpeed + s.all);
    return s;
  }

  function activeSet(state) {
    const scene = state.scene;
    for (let i = 0; i < D.SETS.length; i++) {
      const set = D.SETS[i];
      if (state.scene !== set.scene) continue;
      let all = true;
      for (const cat of Object.keys(set.parts)) {
        if (state.equipped[cat] !== set.parts[cat]) { all = false; break; }
      }
      if (all) return set;
    }
    return null;
  }

  function setProgress(state) {
    return D.SETS.map((set) => {
      let got = state.scene === set.scene ? 1 : 0;
      Object.keys(set.parts).forEach((cat) => {
        if (isOwned(state, cat, set.parts[cat])) got += 1;
      });
      const equipped = Object.keys(set.parts).every((cat) => state.equipped[cat] === set.parts[cat]) && state.scene === set.scene;
      return { set, got, total: 6, equipped };
    });
  }

  /* ================= 背包 ================= */

  function bagCap(state) { return C.bagTiers[state.bag.tier] ? C.bagTiers[state.bag.tier].cap : 30; }
  function gearOwnedCount(state) {
    return Object.keys(state.owned).reduce((n, cat) => n + (state.owned[cat] || []).length, 0);
  }
  function bagUsed(state) { return state.bag.fish.length + gearOwnedCount(state); }
  function bagFree(state) { return Math.max(0, bagCap(state) - bagUsed(state)); }

  function aquariumCap(state) { return (state.bag.tier + 1) * C.aquariumCapPerTier; }

  /** 渔获入包：满包进临时鱼缸。返回去向。 */
  function placeCatch(state, fish) {
    if (bagFree(state) > 0) { state.bag.fish.push(fish); return 'bag'; }
    if (state.aquarium.length < aquariumCap(state)) { state.aquarium.push(fish); return 'aquarium'; }
    return 'lost';
  }

  /* ================= 购买 / 装备 / 养成 ================= */

  function clearBonus(key, base) { return base; }

  function buyItem(state, cat, id) {
    const it = D.itemById[id];
    if (!it || it.cat !== cat) return { ok: false, reason: 'notfound' };
    if (isOwned(state, cat, id)) return { ok: false, reason: 'owned' };
    if (state.magic < it.price) return { ok: false, reason: 'poor', need: it.price };
    state.magic -= it.price;
    if (!state.owned[cat]) state.owned[cat] = [];
    state.owned[cat].push({ id, lv: 1, at: Date.now() });
    return { ok: true, item: it, cost: it.price };
  }

  function equipItem(state, cat, id) {
    if (!isOwned(state, cat, id)) return { ok: false, reason: 'notowned' };
    state.equipped[cat] = id;
    return { ok: true, setId: (activeSet(state) || {}).id || null };
  }

  // 装备槽里若为空则补默认款（存档兼容 / 数据表升级时兜底）
  function ensureEquipped(state) {
    ['rod', 'bait', 'hook', 'line', 'float'].forEach((cat) => {
      const id = state.equipped[cat];
      if (!id || !D.itemById[id] || !isOwned(state, cat, id)) {
        const def = D.ITEMS[cat].find((x) => x.price === 0) || D.ITEMS[cat][0];
        state.equipped[cat] = def.id;
        if (!isOwned(state, cat, def.id)) {
          state.owned[cat] = state.owned[cat] || [];
          state.owned[cat].push({ id: def.id, lv: 1, at: Date.now() });
        }
      }
    });
  }

  function upgradeCost(item, lv) {
    const base = Math.max(120, item.price || 120);
    return Math.round(base * C.upgradeCostRate * lv);
  }

  function upgradeItem(state, cat, id) {
    const it = D.itemById[id];
    const entry = ownedEntry(state, cat, id);
    if (!it || !entry) return { ok: false, reason: 'notowned' };
    const lv = entry.lv || 1;
    if (lv >= C.upgradeMaxLv) return { ok: false, reason: 'max' };
    const cost = upgradeCost(it, lv);
    if (state.magic < cost) return { ok: false, reason: 'poor', need: cost };
    state.magic -= cost;
    entry.lv = lv + 1;
    return { ok: true, lv: entry.lv, cost };
  }

  /** 单件装备当前→下一阶的属性预览（养成页用）。 */
  function upgradePreview(state, cat, id) {
    const it = D.itemById[id];
    if (!it) return null;
    const lv = Math.max(1, itemLv(state, cat, id));
    const scale = (e, lvNow) => {
      const b = (lvNow - 1) / (C.upgradeMaxLv - 1);
      const out = {};
      Object.keys(e || {}).forEach((k) => { out[k] = e[k] * (1 + (k === 'limit' ? 0.4 : k === 'bigFish' || k === 'night' ? 0.2 : 0.3) * b); });
      return out;
    };
    return {
      lv,
      cur: scale(it.eff, lv),
      next: lv < C.upgradeMaxLv ? scale(it.eff, lv + 1) : null,
      cost: lv < C.upgradeMaxLv ? upgradeCost(it, lv) : 0,
    };
  }

  function buyScene(state, sceneId) {
    const sc = D.sceneById[sceneId];
    if (!sc) return { ok: false, reason: 'notfound' };
    if (state.scenesOwned.indexOf(sceneId) >= 0) return { ok: false, reason: 'owned' };
    if (sc.price > 0 && state.magic < sc.price) return { ok: false, reason: 'poor', need: sc.price };
    const prev = D.SCENES[D.SCENES.findIndex((s) => s.id === sceneId) - 1];
    if (prev && state.scenesOwned.indexOf(prev.id) < 0) return { ok: false, reason: 'locked' };
    if (sc.price > 0) state.magic -= sc.price;
    state.scenesOwned.push(sceneId);
    return { ok: true, scene: sc, cost: sc.price };
  }

  function switchScene(state, sceneId) {
    if (state.scenesOwned.indexOf(sceneId) < 0) return { ok: false, reason: 'locked' };
    state.scene = sceneId;
    const sc = D.sceneById[sceneId];
    if (sc.conds.indexOf(state.cond) < 0) state.cond = sc.conds[0];
    return { ok: true, surface: computeSurface(state) };
  }

  function setCond(state, cond) {
    const sc = D.sceneById[state.scene];
    if (sc.conds.indexOf(cond) < 0) return { ok: false, reason: 'cond' };
    state.cond = cond;
    return { ok: true, surface: computeSurface(state) };
  }

  function expandBag(state) {
    const next = C.bagTiers[state.bag.tier + 1];
    if (!next) return { ok: false, reason: 'max' };
    if (state.magic < next.price) return { ok: false, reason: 'poor', need: next.price };
    state.magic -= next.price;
    state.bag.tier += 1;
    return { ok: true, cap: next.cap, cost: next.price, tier: state.bag.tier };
  }

  function unlockAuto(state) {
    if (state.autoUnlocked) return { ok: false, reason: 'owned' };
    if (state.magic < C.autoUnlockPrice) return { ok: false, reason: 'poor', need: C.autoUnlockPrice };
    state.magic -= C.autoUnlockPrice;
    state.autoUnlocked = true;
    return { ok: true, cost: C.autoUnlockPrice };
  }

  /* ================= 抛竿 / 抽鱼 / 结算 ================= */

  /** 计算当前场景时段的视觉色板与光照口径（UI 用，纯函数）。 */
  function computeSurface(state) {
    const sc = D.sceneById[state.scene] || D.SCENES[0];
    return {
      scene: sc.id, cond: state.cond,
      palette: sc.palettes[state.cond] || sc.palettes.day,
      night: state.cond === 'night', rain: state.cond === 'rain',
    };
  }

  /** 鱼池：场景 + 时段筛选，并处理夜光漂 / 饵料等出没门槛。 */
  function poolOf(state) {
    const floatId = state.equipped.float;
    return D.FISH.filter((f) => {
      if (f.scene !== state.scene || f.cond !== state.cond) return false;
      if (f.needFloat && f.needFloat !== floatId) return false;
      return true;
    });
  }

  function rarityWeights(state, pool, stats) {
    const isNight = state.cond === 'night';
    const rareBoost = stats.rare * (isNight ? 1.0 + stats.night : 1) + (isNight ? stats.night * 0.4 : 0);
    const legendBoost = stats.legend + rareBoost * 0.25;
    // 只给当前池里真实存在的稀有度分配权重，避免反复空档重抽
    const present = {};
    pool.forEach((f) => { present[f.rarity] = true; });
    return D.RARITY_ORDER.map((rk) => {
      if (!present[rk]) return 0;
      const base = D.RARITY[rk].weight;
      let w = base;
      if (rk === 'rare') w = base * (1 + rareBoost);
      if (rk === 'epic') w = base * (1 + rareBoost * 0.9);
      if (rk === 'legend') w = base * (1 + legendBoost * 2.2);
      if (rk === 'common') w = base * (1 + (stats.common || 0));
      return w;
    });
  }

  /** 抽鱼：先按稀有度权重定档，再在该档同池鱼中随机。 */
  function rollFish(state, rng) {
    const stats = statsOf(state);
    let pool = poolOf(state);
    if (!pool.length) pool = D.FISH.filter((f) => f.scene === state.scene);
    const r = rng || Math.random;
    let guard = 0;
    while (guard++ < 40) {
      const weights = rarityWeights(state, pool, stats);
      const total = weights.reduce((a, b) => a + b, 0);
      let pick = r() * total;
      let chosen = 'common';
      for (let i = 0; i < weights.length; i++) {
        if (pick < weights[i]) { chosen = D.RARITY_ORDER[i]; break; }
        pick -= weights[i];
      }
      const list = pool.filter((f) => f.rarity === chosen);
      if (list.length) {
        const f = list[Math.floor(r() * list.length)];
        return f;
      }
      // 该档在当前池内无鱼 → 降档重抽
    }
    return pool[0];
  }

  /** 抛竿：扣体力（优先用免费次数），返回咬钩延迟与可起竿窗口。 */
  function cast(state, rng, now) {
    recoverStamina(state, now);
    const stats = statsOf(state);
    if (state.stamina.v < C.castStaminaCost && state.freeCasts <= 0) {
      return { ok: false, reason: 'noStamina' };
    }
    if (state.freeCasts > 0) state.freeCasts -= 1;
    else state.stamina.v -= C.castStaminaCost;

    const r = rng || Math.random;
    // 咬钩延迟：2.4s–5.0s，饵料提速；阴雨扰动使咬钩更快（+15%）
    const base = 2400 + r() * 2600;
    const condBoost = state.cond === 'rain' ? 0.15 : 0;
    const biteAfter = Math.round(base / (1 + stats.biteSpeed + condBoost));
    const windowMs = Math.round(stats.window);
    const fish = rollFish(state, r);

    state.stats.casts += 1;
    state.stats.plays += 1;
    return {
      ok: true, biteAfter, windowMs, fish, stats,
      limit: stats.limit, pull: stats.pull,
      surface: computeSurface(state),
    };
  }

  /** 溜鱼数值（每帧调用，dt 秒）。返回 {tension, progress, broken, landed}。 */
  function reelStep(rt, dt) {
    const elapsed = rt.elapsed;
    // 鱼的挣扎节奏：正弦起伏，纯函数、零随机，帧率无关于结果
    const burst = 1 + 0.55 * Math.sin(elapsed * 2.4) * Math.sin(elapsed * 0.7 + 1);
    const hold = rt.holding;
    if (hold) {
      rt.tension += rt.rise * burst * dt;
      if (rt.tension <= rt.softLimit) rt.progress += rt.gain * dt;
      else rt.progress -= rt.gain * 0.25 * dt;
    } else {
      rt.tension -= rt.decay * dt;
      rt.progress -= rt.gain * 0.18 * dt;
    }
    rt.tension = clamp(rt.tension, 0, 120);
    rt.progress = clamp(rt.progress, 0, 100);
    rt.elapsed += dt;
    if (rt.tension >= rt.limit) return { tension: rt.tension, progress: rt.progress, broken: true, landed: false, elapsed };
    if (rt.progress >= 100) return { tension: rt.tension, progress: rt.progress, broken: false, landed: true, elapsed };
    if (rt.elapsed >= rt.maxTime) return { tension: rt.tension, progress: rt.progress, broken: false, landed: false, escaped: true, elapsed };
    return { tension: rt.tension, progress: rt.progress, broken: false, landed: false };
  }

  /** 创建一次溜鱼的运行时参数。 */
  function makeReelRuntime(castInfo, fish) {
    const river = fish.river || 1;
    const rar = D.RARITY[fish.rarity] || D.RARITY.common;
    const bigReduce = 1 - (castInfo.stats.bigFish || 0) * 0.45;
    return {
      tension: 18,
      progress: 0,
      rise: (17 + river * 4.2) * rar.strength * 0.55 * bigReduce,
      decay: 15,
      gain: 15.5 * (castInfo.stats.pull || 1),
      limit: castInfo.limit || 100,
      softLimit: Math.min((castInfo.limit || 100) - 12, 86),
      maxTime: 24,
      elapsed: 0,
      holding: false,
    };
  }

  /** 结算一条鱼：魔力、体型、图鉴、订单、成就、统计。 */
  function settleCatch(state, fish, ctx, rng) {
    const stats = ctx && ctx.stats ? ctx.stats : statsOf(state);
    const r = rng || Math.random;
    const capSize = Math.max(0.01, fish.size[1] - fish.size[0]);
    const size = round2(fish.size[0] + r() * capSize);
    const sizeFactor = 0.8 + (size - fish.size[0]) / capSize * 0.8;
    const magic = Math.max(6, Math.round(fish.value * sizeFactor * (1 + (stats.all || 0))));
    const weight = round2(size * (0.02 + r() * 0.03));

    const entry = {
      uid: 'f' + Date.now().toString(36) + Math.floor(r() * 1e4).toString(36),
      id: fish.id, name: fish.name, rarity: fish.rarity,
      size, weight, magic, scene: state.scene, cond: state.cond,
      at: Date.now(),
    };
    const dest = placeCatch(state, entry);
    state.magic += magic;
    state.stats.catches += 1;
    state.stats.magicEarned += magic;
    state.stats.curStreak += 1;
    state.stats.bestStreak = Math.max(state.stats.bestStreak, state.stats.curStreak);
    if (fish.rarity === 'rare' || fish.rarity === 'epic' || fish.rarity === 'legend') state.stats.rareCaught += 1;
    if (fish.rarity === 'epic') state.stats.epicCaught += 1;
    if (fish.rarity === 'legend') state.stats.legendCaught += 1;
    if (state.cond === 'night') state.stats.nightCatches += 1;
    if (state.cond === 'rain') state.stats.rainCatches += 1;
    if (size >= 60) state.stats.bigCaught += 1;

    const isNew = !state.album[fish.id];
    const prev = state.album[fish.id];
    state.album[fish.id] = {
      count: (prev ? prev.count : 0) + 1,
      first: prev ? prev.first : Date.now(),
      best: Math.max(prev ? prev.best : 0, magic),
      bestSize: Math.max(prev ? prev.bestSize || 0 : 0, size),
      scene: state.scene,
    };

    const orderHits = bumpOrders(state, fish.id);

    return {
      entry, dest, magic, isNew, size, weight,
      albumCount: Object.keys(state.album).length,
      orderHits,
      bagText: { used: bagUsed(state), cap: bagCap(state) },
    };
  }

  /** 起竿超时 / 断线：只记统计，不产生渔获。 */
  function settleMiss(state, kind) {
    state.stats.curStreak = 0;
    if (kind === 'break') state.stats.breaks += 1;
    else state.stats.missed += 1;
  }

  /* ================= 每日订单 ================= */

  function orderSeed(state, dayKey) {
    let h = 2166136261;
    const s = dayKey + '|' + state.scene;
    for (let i = 0; i < s.length; i++) { h ^= s.charCodeAt(i); h = Math.imul(h, 16777619); }
    return h >>> 0;
  }

  function ensureOrders(state, now) {
    const key = todayKey(now);
    if (state.orders && state.orders.day === key && state.orders.list.length) return state.orders;
    const r = mulberry32(orderSeed(state, key));
    const pool = D.FISH.filter((f) => state.scenesOwned.indexOf(f.scene) >= 0);
    const pickPool = pool.length >= 3 ? pool.slice() : D.FISH.slice();
    const list = [];
    const used = {};
    for (let i = 0; i < 3; i++) {
      let idx = Math.floor(r() * pickPool.length);
      let guard = 0;
      while (used[pickPool[idx].id] && guard++ < 30) idx = Math.floor(r() * pickPool.length);
      const f = pickPool[idx];
      used[f.id] = true;
      const need = 1 + Math.floor(r() * 3);
      const reward = Math.round(f.value * 2.6 * need);
      list.push({ id: f.id, need, got: 0, reward, claimed: false });
    }
    state.orders = { day: key, list };
    return state.orders;
  }

  function bumpOrders(state, fishId) {
    if (!state.orders || !state.orders.list) return 0;
    let hits = 0;
    state.orders.list.forEach((o) => {
      if (o.id === fishId && !o.claimed && o.got < o.need) { o.got += 1; hits += 1; }
    });
    return hits;
  }

  function claimOrder(state, index) {
    const o = state.orders && state.orders.list[index];
    if (!o) return { ok: false, reason: 'missing' };
    if (o.claimed) return { ok: false, reason: 'claimed' };
    if (o.got < o.need) return { ok: false, reason: 'progress' };
    o.claimed = true;
    state.magic += o.reward;
    state.stats.magicEarned += o.reward;
    state.stats.ordersDone += 1;
    return { ok: true, reward: o.reward };
  }

  function allOrdersDone(state) {
    const list = (state.orders && state.orders.list) || [];
    return list.length > 0 && list.every((o) => o.claimed);
  }

  /** 24 单额外奖励：全部订单领完后可领一次 ✦300 */
  function claimOrderBonus(state) {
    if (!allOrdersDone(state)) return { ok: false, reason: 'progress' };
    if (state.orders.bonusClaimed) return { ok: false, reason: 'claimed' };
    state.orders.bonusClaimed = true;
    state.magic += C.orderAllBonus;
    state.stats.magicEarned += C.orderAllBonus;
    return { ok: true, reward: C.orderAllBonus };
  }

  /** 从背包放生一尾（清理容量，无二次收益：魔力已在钓获时入账） */
  function releaseFromBag(state, uid) {
    const idx = state.bag.fish.findIndex((f) => f.uid === uid);
    if (idx < 0) return { ok: false, reason: 'missing' };
    const fish = state.bag.fish.splice(idx, 1)[0];
    return { ok: true, fish };
  }

  /** 从临时鱼缸取回背包（受空位限制） */
  function takeFromAquarium(state, uid) {
    const idx = state.aquarium.findIndex((f) => f.uid === uid);
    if (idx < 0) return { ok: false, reason: 'missing' };
    if (bagFree(state) <= 0) return { ok: false, reason: 'full' };
    const fish = state.aquarium.splice(idx, 1)[0];
    state.bag.fish.push(fish);
    return { ok: true, fish };
  }

  /** 放生临时鱼缸里的鱼 */
  function releaseFromAquarium(state, uid) {
    const idx = state.aquarium.findIndex((f) => f.uid === uid);
    if (idx < 0) return { ok: false, reason: 'missing' };
    const fish = state.aquarium.splice(idx, 1)[0];
    return { ok: true, fish };
  }

  function takeAllFromAquarium(state) {
    let moved = 0;
    while (state.aquarium.length && bagFree(state) > 0) {
      state.bag.fish.push(state.aquarium.shift());
      moved += 1;
    }
    return { ok: moved > 0, moved, left: state.aquarium.length };
  }

  /* ================= 签到 ================= */

  function signState(state, now) {
    const key = todayKey(now);
    const sign = state.sign;
    return {
      day: key,
      idx: sign.idx || 0,
      claimedToday: sign.last === key,
      nextIndex: sign.last === key ? sign.idx : sign.idx,
      cycle: sign.cycle || 0,
    };
  }

  function claimSign(state, now) {
    const key = todayKey(now);
    if (state.sign.last === key) return { ok: false, reason: 'claimed' };
    const idx = state.sign.idx % C.signInRewards.length;
    const rw = C.signInRewards[idx];
    state.sign.last = key;
    state.sign.idx = (idx + 1) % C.signInRewards.length;
    if (state.sign.idx === 0) {
      state.sign.cycle += 1;
      state.stats.signFull += 1;
    }
    if (rw.type === 'magic') { state.magic += rw.value; state.stats.magicEarned += rw.value; }
    else if (rw.type === 'stamina') { state.stamina.v = Math.min(C.staminaMax, state.stamina.v + rw.value); }
    else if (rw.type === 'bait') { state.freeCasts += rw.value; }
    else if (rw.type === 'chest') { state.magic += rw.value; state.freeCasts += 4; state.stats.magicEarned += rw.value; }
    return { ok: true, reward: rw, idx };
  }

  /* ================= 好友 / 赠饵 ================= */

  function friendState(state, now) {
    const key = todayKey(now);
    if (!state.friends || state.friends.day !== key) {
      state.friends = { day: key, sent: 0, received: state.friends && state.friends.received ? state.friends.received.slice(0, C.giftReceiveCap) : [] };
    }
    return state.friends;
  }

  /** 好友在线态与鱼塘陈列（按 seed + 小时确定性生成，不用随机）。 */
  function friendView(state, now) {
    const hour = Math.floor((now == null ? Date.now() : now) / 3600000);
    return D.FRIENDS.map((f) => {
      const h = (f.seed * 31 + hour) % 24;
      const online = h % 3 !== 0;
      const pond = [0, 1, 2].map((i) => {
        const x = (f.seed * 7 + i * 13 + hour) % D.FISH.length;
        return D.FISH[x];
      });
      return { ...f, online, pond };
    });
  }

  function giftBait(state, friendId, now) {
    const fs = friendState(state, now);
    if (fs.sent >= C.giftDailyLimit) return { ok: false, reason: 'limit' };
    fs.sent += 1;
    state.stats.giftsSent += 1;
    return { ok: true, left: C.giftDailyLimit - fs.sent };
  }

  function receiveGifts(state, now) {
    const fs = friendState(state, now);
    const got = Math.min(fs.received.length, C.giftReceiveCap);
    if (!got) return { ok: false, reason: 'empty' };
    fs.received.splice(0, got);
    state.freeCasts += got;
    return { ok: true, count: got };
  }

  /* ================= 排行 / 赛季 ================= */

  const SEASON_BASE_DAY = Math.floor(new Date(2026, 0, 1, 0, 0, 0).getTime() / DAY_MS);

  function seasonInfo(state, now) {
    const day = dayNumber(now);
    const id = Math.floor((day - SEASON_BASE_DAY) / C.seasonDays);
    const dayIdx = ((day - SEASON_BASE_DAY) % C.seasonDays + C.seasonDays) % C.seasonDays;
    return {
      id, ordinal: Math.max(1, id + 1), dayIndex: dayIdx,
      leftDays: C.seasonDays - dayIdx, endsInMs: (C.seasonDays - dayIdx) * DAY_MS,
    };
  }

  function ensureSeason(state, now) {
    const info = seasonInfo(state, now);
    if (!state.season || state.season.id !== info.id) {
      state.season = { id: info.id, catches: 0, claimed: false };
    }
    return state.season;
  }

  function rankBoard(state, now) {
    const info = seasonInfo(state, now);
    const season = ensureSeason(state, now);
    const rows = D.RIVALS.map((r, i) => {
      const seedVar = ((info.id * 97 + i * 31) % 17) - 8;
      const catches = Math.max(0, Math.round(r.base + r.growth * info.dayIndex + seedVar));
      return { id: r.id, name: r.name, tag: r.tag, tone: r.tone, catches, reward: D.RANK_REWARDS[i] || 50, me: false };
    });
    rows.push({ id: 'me', name: '我的渔场', tag: '我', tone: 'deep', catches: season.catches, reward: 0, me: true });
    rows.sort((a, b) => b.catches - a.catches);
    rows.forEach((r, i) => { r.rank = i + 1; r.reward = D.RANK_REWARDS[i] || 50; });
    const me = rows.find((r) => r.me);
    return { info, rows, me, season };
  }

  function addSeasonCatch(state, now) {
    const s = ensureSeason(state, now);
    s.catches += 1;
  }

  /* ================= 成就 ================= */

  function metricValue(state, metric) {
    switch (metric) {
      case 'albumCount': return Object.keys(state.album).length;
      case 'sceneCount': return state.scenesOwned.length;
      case 'setActive': return activeSet(state) ? 1 : 0;
      case 'gearOwned': return gearOwnedCount(state);
      case 'maxLv': return Object.keys(state.owned).reduce((m, cat) => Math.max(m, ...state.owned[cat].map((x) => x.lv || 1)), 1);
      case 'bagTier': return state.bag.tier;
      default: return state.stats[metric] || 0;
    }
  }

  function achView(state) {
    return D.ACH.map((a) => {
      const prog = Math.min(a.target, metricValue(state, a.metric));
      const rec = state.ach[a.id] || null;
      const done = prog >= a.target;
      return {
        def: a, prog, done,
        claimed: !!(rec && rec.claimed),
        claimable: done && !(rec && rec.claimed),
      };
    });
  }

  function claimAch(state, id) {
    const view = achView(state).find((v) => v.def.id === id);
    if (!view) return { ok: false, reason: 'notfound' };
    if (!view.claimable) return { ok: false, reason: 'locked' };
    state.ach[id] = { claimed: true, at: Date.now() };
    state.magic += view.def.reward;
    state.stats.magicEarned += view.def.reward;
    return { ok: true, reward: view.def.reward };
  }

  /* ================= 图鉴分区 ================= */

  function albumView(state) {
    const conds = [
      { id: 'day', name: '白天', icon: 'sun' },
      { id: 'night', name: '夜钓', icon: 'moon' },
      { id: 'rain', name: '阴雨', icon: 'rain' },
    ];
    const byScene = {};
    D.FISH.forEach((f) => {
      const key = f.scene;
      if (!byScene[key]) byScene[key] = { scene: D.sceneById[f.scene], conds: {} };
      if (!byScene[key].conds[f.cond]) byScene[key].conds[f.cond] = { cond: f.cond, list: [] };
      byScene[key].conds[f.cond].list.push({ fish: f, got: !!state.album[f.id] });
    });
    const groups = [];
    D.SCENES.forEach((sc) => {
      const g = byScene[sc.id];
      if (!g) return;
      conds.forEach((c) => {
        const bucket = g.conds[c.id];
        if (!bucket) return;
        const got = bucket.list.filter((x) => x.got).length;
        groups.push({
          sceneId: sc.id, sceneName: sc.name, cond: c.id, condName: c.name, icon: c.icon,
          total: bucket.list.length, got, list: bucket.list,
          unlocked: state.scenesOwned.indexOf(sc.id) >= 0,
        });
      });
    });
    return groups;
  }

  /* ================= 存档初始化 ================= */

  function defaultState(now) {
    const t = now == null ? Date.now() : now;
    return {
      v: 1, magic: 3200, freeCasts: 3,
      stamina: { v: C.staminaMax, at: t },
      scene: 'bay', cond: 'day',
      scenesOwned: ['bay'],
      equipped: { rod: 'bamboo', bait: 'bread', hook: 'iron', line: 'cotton', float: 'reed' },
      owned: {
        rod: [{ id: 'bamboo', lv: 1 }], bait: [{ id: 'bread', lv: 1 }],
        hook: [{ id: 'iron', lv: 1 }], line: [{ id: 'cotton', lv: 1 }], float: [{ id: 'reed', lv: 1 }],
      },
      autoUnlocked: false, auto: false,
      bag: { tier: 0, fish: [] },
      aquarium: [],
      album: {},
      orders: { day: '', list: [] },
      sign: { idx: 0, last: '', cycle: 0 },
      friends: { day: '', sent: 0, received: [] },
      season: { id: 0, catches: 0, claimed: false },
      ach: {},
      stats: {
        casts: 0, catches: 0, breaks: 0, missed: 0, plays: 0,
        magicEarned: 0, rareCaught: 0, epicCaught: 0, legendCaught: 0,
        nightCatches: 0, rainCatches: 0, bigCaught: 0,
        bestStreak: 0, curStreak: 0, ordersDone: 0, giftsSent: 0, signFull: 0,
      },
      settings: { reduceMotion: false },
      createdAt: t,
    };
  }

  /** 存档补全：老存档缺字段时按默认值补齐，避免读到 undefined 崩页。 */
  function hydrate(state, now) {
    const base = defaultState(now);
    const out = state && typeof state === 'object' ? state : base;
    const merge = (k, v) => { if (out[k] == null) out[k] = v; };
    merge('magic', base.magic); merge('freeCasts', 0); merge('stamina', base.stamina);
    merge('scene', 'bay'); merge('cond', 'day'); merge('scenesOwned', base.scenesOwned);
    merge('equipped', base.equipped); merge('owned', base.owned);
    merge('bag', base.bag); merge('aquarium', []);
    merge('album', {}); merge('orders', base.orders); merge('sign', base.sign);
    merge('friends', base.friends); merge('season', base.season); merge('ach', {});
    merge('stats', base.stats); merge('settings', base.settings);
    out.stats = Object.assign({}, base.stats, out.stats || {});
    out.settings = Object.assign({}, base.settings, out.settings || {});
    ensureEquipped(out);
    if (out.scenesOwned.indexOf(out.scene) < 0) out.scene = 'bay';
    const sc = D.sceneById[out.scene];
    if (sc && sc.conds.indexOf(out.cond) < 0) out.cond = sc.conds[0];
    return out;
  }

  G.FTEngine = {
    mulberry32, todayKey, dayNumber, clamp, recoverStamina, staminaNextIn, refillStamina,
    statsOf, activeSet, setProgress, isOwned, itemLv, upgradeCost, upgradePreview,
    bagCap, bagUsed, bagFree, gearOwnedCount, aquariumCap, placeCatch,
    buyItem, equipItem, ensureEquipped, upgradeItem, buyScene, switchScene, setCond,
    expandBag, unlockAuto,
    computeSurface, poolOf, rollFish, cast, makeReelRuntime, reelStep,
    settleCatch, settleMiss,
    ensureOrders, bumpOrders, claimOrder, allOrdersDone, claimOrderBonus,
    releaseFromBag, takeFromAquarium, releaseFromAquarium, takeAllFromAquarium,
    signState, claimSign,
    friendState, friendView, giftBait, receiveGifts,
    seasonInfo, ensureSeason, rankBoard, addSeasonCatch,
    metricValue, achView, claimAch, albumView,
    defaultState, hydrate,
  };
})(typeof window !== 'undefined' ? window : globalThis);
