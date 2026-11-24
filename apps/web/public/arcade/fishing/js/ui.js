/* 钓鱼玩法 · 界面层
 * 只负责把 state 渲染成 HTML 片段，不持有游戏运行时状态；交互统一走 data-act 事件委托。
 */
(function (G) {
  'use strict';
  const D = G.FTData;
  const E = G.FTEngine;

  /* ---------------- 基础工具 ---------------- */
  const esc = (s) => String(s == null ? '' : s).replace(/[&<>"']/g, (c) => ({ '&': '&amp;', '<': '&lt;', '>': '&gt;', '"': '&quot;', "'": '&#39;' }[c]));
  const fmt = (n) => Math.round(n).toLocaleString('zh-CN');
  const pct = (v) => Math.round(v * 100) + '%';
  const icon = (n, cls) => `<svg class="ic${cls ? ' ' + cls : ''}"><use href="#i-${n}"/></svg>`;
  const rarTag = (r) => `<span class="rar r-${r}">${D.RARITY[r].name}</span>`;
  const tone = (t) => 'tone-' + (t || 'sky');

  const EFF = {
    window:   { k: '起竿窗口', f: (v) => '+' + Math.round(v) + 'ms', good: true },
    biteSpeed:{ k: '咬钩速度', f: (v) => '+' + pct(v), good: true },
    hookRate: { k: '中钩率',   f: (v) => '+' + pct(v), good: true },
    escape:   { k: '脱钩率',   f: (v) => '-' + pct(v), good: true },
    rare:     { k: '稀有鱼概率', f: (v) => '+' + pct(v), good: true },
    legend:   { k: '传说鱼概率', f: (v) => '+' + pct(v), good: true },
    limit:    { k: '断线阈值', f: (v) => '+' + Math.round(v) + '点', good: true },
    alert:    { k: '鱼群警觉', f: (v) => '-' + pct(v), good: true },
    pull:     { k: '收线效率', f: (v) => '+' + pct(v), good: true },
    bigFish:  { k: '大鱼承受', f: (v) => '+' + pct(v), good: true },
    night:    { k: '夜钓加成', f: (v) => '+' + pct(v), good: true },
    common:   { k: '常见鱼概率', f: (v) => '+' + pct(v), good: true },
  };

  function attrLine(eff) {
    const keys = Object.keys(eff || {});
    if (!keys.length) return '标准款 · 无额外加成';
    return keys.map((k) => (EFF[k] ? EFF[k].k + ' ' + EFF[k].f(eff[k]) : k)).join(' · ');
  }

  function condName(c) { return c === 'night' ? '夜钓' : c === 'rain' ? '阴雨' : '白天'; }
  function condIcon(c) { return c === 'night' ? 'moon' : c === 'rain' ? 'rain' : 'sun'; }
  function sceneTone(id) {
    return id === 'bay' ? 'sky' : id === 'gorge' ? 'lilac' : id === 'abyss' ? 'deep' : 'mint';
  }

  /* ---------------- 主钓场 ---------------- */
  function rodColor(id) {
    return id === 'dragon' ? '#d4af7a' : id === 'moon' ? '#9a8ac8' : id === 'stardust' ? '#8ab4dd' : '#b07838';
  }
  function floatClass(id) {
    return id === 'glow' ? 'glow' : id === 'crystal' ? 'crystal' : id === 'balsa' ? 'alt' : '';
  }

  function pondWater(state, ctx) {
    const sc = D.sceneById[state.scene];
    const p = sc.palettes[state.cond];
    const cond = state.cond === 'night' ? 'night' : state.cond === 'rain' ? 'rain' : '';
    const rod = D.itemById[state.equipped.rod] || {};
    const fl = D.itemById[state.equipped.float] || {};
    const cls = floatClass(state.equipped.float);
    const grip = rodColor(state.equipped.rod);

    const deco = sc.id === 'bay'
      ? `<div class="deco-palm p1">${icon('palm')}</div><div class="deco-palm p2">${icon('palm')}</div>`
      : sc.id === 'gorge'
        ? `<div class="deco-palm p1" style="color:#e8f4ff;opacity:.5">${icon('scene')}</div><div class="deco-palm p2" style="color:#e8f4ff;opacity:.35">${icon('scene')}</div>`
        : sc.id === 'abyss'
          ? `<div class="deco-palm p1" style="color:#cfe0ff;opacity:.28">${icon('moon')}</div><div class="deco-palm p2" style="color:#9fb8e8;opacity:.22">${icon('scene')}</div>`
          : `<div class="deco-palm p1">${icon('palm')}</div><div class="deco-palm p2" style="color:#3fb0a8">${icon('fish')}</div>`;

    return `
      <div class="water ${cond} ${ctx.phase === 'bite' ? 'biting' : ''}" id="water" style="background:linear-gradient(180deg,${p[0]} 0%,${p[1]} 45%,${p[2]} 100%)">
        <div class="sun-orb"></div>
        <div class="cloud c1"></div><div class="cloud c2"></div>
        ${deco}
        <div class="auto-badge ${ctx.autoOn ? 'on' : ''}" id="autoBadge">${icon('bolt')}自动垂钓中</div>
        <svg class="rod-svg" viewBox="0 0 230 330" aria-hidden="true">
          <path d="M20 320 L150 60" stroke="#3a2a18" stroke-width="13" stroke-linecap="round" opacity=".35"/>
          <path d="M20 320 L150 60" stroke="${grip}" stroke-width="12" stroke-linecap="round"/>
          <path d="M20 320 L150 60" stroke="#fff" stroke-width="3" stroke-linecap="round" opacity=".35"/>
          <circle cx="70" cy="220" r="6" fill="#6a4a28"/>
          <circle cx="105" cy="150" r="6" fill="#6a4a28"/>
          <circle cx="135" cy="90" r="6" fill="#6a4a28"/>
          <path class="rod-line" d="M150 60 L192 150"/>
          <circle cx="34" cy="296" r="20" fill="#8ab4dd" stroke="#4a7ab0" stroke-width="4"/>
          <circle cx="34" cy="296" r="8" fill="#4a7ab0"/>
        </svg>
        <div class="float-wrap ${ctx.phase === 'bite' ? 'bite' : ''}" id="floatWrap">
          <div class="float ${cls} bob"></div>
        </div>
        <div class="ripple r1"></div><div class="ripple r2"></div><div class="ripple r3"></div>

        <div class="bubble ${ctx.phase === 'bite' || ctx.phase === 'reel' ? 'hide' : ''}" id="pondBubble">${ctx.bubble || '点「抛竿」开始垂钓'}</div>

        <div class="bite-panel ${ctx.phase === 'bite' ? 'on' : ''}" id="bitePanel">
          <div class="bite-ring">
            <svg viewBox="0 0 100 100">
              <circle class="track" cx="50" cy="50" r="44"/>
              <circle class="prog" id="biteProg" cx="50" cy="50" r="44" stroke-dasharray="276.46" stroke-dashoffset="0"/>
            </svg>
            <button class="bite-btn" data-act="hook">起竿</button>
          </div>
          <div class="bite-hint" id="biteHint">鱼讯出现 · 窗口内起竿</div>
        </div>

        <div class="fab-col">
          <button class="fab" data-act="toggleAuto">
            <span class="fb-ic">${icon('bolt')}${state.autoUnlocked ? '' : `<span class="fb-lock">${icon('lock')}</span>`}</span>
            <span class="fb-lb">自动</span>
          </button>
          <button class="fab" data-act="openView" data-view="aquarium">
            <span class="fb-ic" style="color:var(--lilac-deep)">${icon('aqua')}${state.aquarium.length ? `<span class="fb-bad">${state.aquarium.length}</span>` : ''}</span>
            <span class="fb-lb">鱼缸</span>
          </button>
        </div>
      </div>`;
  }

  function pondAside(state) {
    const list = (state.orders && state.orders.list) || [];
    const rows = list.slice(0, 3).map((o) => {
      const f = D.fishById[o.id] || { name: '?' };
      const done = o.got >= o.need;
      return `<div class="order-row ${done ? 'done' : ''}">
        <div class="or-ic tone-${sceneTone((D.fishById[o.id] || {}).scene || 'bay')}">${icon('fish')}</div>
        <div class="or-nm">${esc(f.name)}</div>
        <div class="or-pr">${o.got}/${o.need}${done && !o.claimed ? ' ✓' : ''}</div>
      </div>`;
    }).join('');
    const claimedCount = list.filter((o) => o.claimed).length;

    return `
      <div class="pond-aside">
        <div class="order-card">
          <div class="oc-t">每日订单 <span>${claimedCount}/${list.length}</span></div>
          ${rows || '<div class="order-row"><div class="or-nm">今日订单生成中…</div></div>'}
          <button class="oc-more" data-act="openView" data-view="orders">订单详情 ›</button>
        </div>

        <div class="aside-head">快捷入口 <span>${D.sceneById[state.scene].name}</span></div>
        <div class="quick-grid">
          <button class="q" data-act="openView" data-view="stamina">
            <span class="q-ic tone-sun">${icon('bolt')}</span>
            <span class="q-meta"><span class="q-nm">体力签到</span><span class="q-sub">${state.stamina.v}/${D.CONST.staminaMax}${state.sign.last ? '' : ' · 待签到'}</span></span>
            ${state.sign.last ? '' : '<span class="q-dot"></span>'}
          </button>
          <button class="q" data-act="openView" data-view="friends">
            <span class="q-ic tone-lilac">${icon('friend')}</span>
            <span class="q-meta"><span class="q-nm">好友鱼塘</span><span class="q-sub">待收 ${E.friendState(state).received.length} · 赠饵 ${D.CONST.giftDailyLimit - E.friendState(state).sent}</span></span>
            ${E.friendState(state).received.length ? '<span class="q-dot"></span>' : ''}
          </button>
          <button class="q" data-act="openView" data-view="rank">
            <span class="q-ic tone-gold">${icon('trophy')}</span>
            <span class="q-meta"><span class="q-nm">赛季排行</span><span class="q-sub">第 ${ctxRank(state)} 名</span></span>
          </button>
          <button class="q" data-act="openView" data-view="ach">
            <span class="q-ic tone-mint">${icon('medal')}</span>
            <span class="q-meta"><span class="q-nm">成就徽章</span><span class="q-sub">${achDone(state)}/${D.ACH.length} 达成</span></span>
            ${achClaimable(state) ? '<span class="q-dot"></span>' : ''}
          </button>
          <button class="q" data-act="openView" data-view="aquarium">
            <span class="q-ic tone-sky">${icon('aqua')}</span>
            <span class="q-meta"><span class="q-nm">临时鱼缸</span><span class="q-sub">${state.aquarium.length}/${E.aquariumCap(state)} 尾</span></span>
          </button>
          <button class="q" data-act="openView" data-view="album">
            <span class="q-ic tone-coral">${icon('album')}</span>
            <span class="q-meta"><span class="q-nm">鱼类图鉴</span><span class="q-sub">${Object.keys(state.album).length}/${D.FISH.length} 收录</span></span>
          </button>
        </div>

        <div class="aside-head">已装备 <span>点格更换</span></div>
        <div class="gear-strip">${gearSlots(state)}</div>
      </div>`;
  }

  function gearSlots(state) {
    const slots = [
      { cat: 'scene', lb: '场景', ic: 'scene', tone: 'sky', name: D.sceneById[state.scene].name },
      ...D.CATS.map((c) => ({
        cat: c.id, lb: c.name, ic: c.icon, id: state.equipped[c.id],
        tone: { rod: 'sun', bait: 'mint', hook: 'coral', line: 'lilac', float: 'sky' }[c.id],
        name: (D.itemById[state.equipped[c.id]] || {}).name || '',
      }))
    ];
    return slots.map((s) => {
      if (s.cat === 'scene') {
        return `<button class="gslot" data-act="openSceneSheet">
          <span class="gs-ic ${tone(s.tone)}">${icon(s.ic)}</span>
          <span class="gs-lb">场景</span><span class="gs-dot"></span>
        </button>`;
      }
      const lv = E.itemLv(state, s.cat, s.id) || 1;
      return `<button class="gslot" data-act="openItem" data-cat="${s.cat}" data-id="${s.id}" title="${esc(s.name)}">
        <span class="gs-ic ${tone(s.tone)}">${icon(s.ic)}</span>
        <span class="gs-lv">${lv > 1 ? 'Lv' + lv : ''}</span>
        <span class="gs-lb">${s.lb}</span>
        <span class="gs-dot"></span>
      </button>`;
    }).join('');
  }

  function ctxRank(state) {
    if (G.__FT_RANK && G.__FT_RANK.me) return G.__FT_RANK.me.rank || '-';
    try { return E.rankBoard(state, Date.now()).me.rank; } catch (e) { return '-'; }
  }
  function achDone(state) { return E.achView(state).filter((v) => v.done).length; }
  function achClaimable(state) { return E.achView(state).filter((v) => v.claimable).length; }

  function renderPond(state, ctx) {
    const info = E.computeSurface(state);
    const busy = ctx.phase === 'wait' || ctx.phase === 'reel';
    const isBite = ctx.phase === 'bite';
    const btnCast = isBite
      ? `<button class="cta cta-gold" data-act="hook">${icon('net')}起竿！</button>`
      : `<button class="cta cta-primary" data-act="cast" ${busy ? 'disabled' : ''}>${icon('rod')}${ctx.phase === 'wait' ? '等待鱼讯…' : ctx.phase === 'reel' ? '溜鱼中…' : '抛竿'}</button>`;
    const tip = state.freeCasts > 0 ? `免费抛竿 ${state.freeCasts} 次` : `体力 ${state.stamina.v}/${D.CONST.staminaMax}`;
    return `<div class="view-inner">
      <div class="pond">
        ${pondWater(state, ctx)}
        ${pondAside(state)}
        <div class="pond-cta">
          ${btnCast}
          <button class="cta cta-gold" data-act="openView" data-view="gear">更换装备</button>
        </div>
      </div>
      <div class="cap-sub" style="padding:0 16px 10px;background:#fff;margin-top:-1px">
        <span>${esc(D.sceneById[state.scene].name)} · ${condName(state.cond)}</span>
        <span>${tip}</span>
        <span>背包 ${E.bagUsed(state)}/${E.bagCap(state)} · 渔运 ${state.luck || 0}</span>
        <span>${ctx.phase === 'idle' ? '窗口 ' + Math.round(E.statsOf(state).window) + 'ms' : ''}</span>
      </div>
      <div class="cap-sub" style="padding:0 16px 8px;background:#fff;gap:6px;flex-wrap:wrap">
        ${D.BAITS.map((b) => {
          const on = state.baitId === b.id;
          const have = (state.baits && state.baits[b.id]) || 0;
          const label = b.price === 0 ? b.name : `${b.name} ×${have}`;
          return `<button class="stab" style="padding:4px 10px;font-size:12px${on ? ';border-color:var(--gold);color:var(--gold-deep)' : ''}" data-act="${on || (b.price > 0 && have < 1) ? 'openView' : 'selectBait'}" ${on || (b.price > 0 && have < 1) ? `data-view="shop" data-cat="bait"` : `data-id="${b.id}"`}>${label}</button>`;
        }).join('')}
      </div>
    </div>`;
  }

  /* ---------------- 商店 ---------------- */
  function renderShop(state, ctx) {
    const cat = ctx.shopCat || 'rod';
    const tabs = [
      { id: 'scene', name: '场景', ic: 'scene' },
      ...D.CATS.map((c) => ({ id: c.id, name: c.name, ic: c.icon })),
      { id: 'bait', name: '打窝饵', ic: 'bait' },
      { id: 'bag', name: '背包扩容', ic: 'bag' },
      { id: 'auto', name: '自动垂钓', ic: 'bolt' },
    ];
    const tabsHtml = tabs.map((t) => `<button class="stab ${cat === t.id ? 'on' : ''}" data-act="shopCat" data-cat="${t.id}">${icon(t.ic)}${t.name}</button>`).join('');

    let body = '';
    if (cat === 'scene') body = shopScene(state);
    else if (cat === 'bait') body = shopBait(state);
    else if (cat === 'bag') body = shopBag(state);
    else if (cat === 'auto') body = shopAuto(state);
    else body = shopItems(state, cat);

    return `<div class="view-inner">
      <div class="page-head">
        <div>
          <div class="ph-t">渔具商店</div>
          <div class="ph-s">全场统一使用站点魔力 ✦ · 渔场只消耗不返还（渔获/订单/成就奖励仅为图鉴估值）</div>
        </div>
      </div>
      <div class="tabs-scroll" id="shopTabs">${tabsHtml}</div>
      ${body}
    </div>`;
  }

  function shopItems(state, cat) {
    const items = D.ITEMS[cat];
    const equipped = state.equipped[cat];
    const rendered = items.map((it) => {
      const owned = E.isOwned(state, cat, it.id);
      const lv = E.itemLv(state, cat, it.id) || 1;
      const isOn = equipped === it.id;
      let act;
      if (isOn) act = `<span class="act equipped">使用中</span>`;
      else if (owned) act = `<button class="act owned" data-act="equip" data-cat="${cat}" data-id="${it.id}">装备</button>`;
      else act = `<button class="act buy" data-act="openItem" data-cat="${cat}" data-id="${it.id}">${icon('sparkle')} ${fmt(it.price)}</button>`;
      return `<div class="item-card" data-act="openItem" data-cat="${cat}" data-id="${it.id}">
        <div class="ic-box ${tone(catTone(cat))}">${icon(D.catById[cat].icon)}</div>
        <div class="im">
          <div class="im-top"><span class="im-nm">${esc(it.name)}</span>${rarTag(it.rarity)}${owned ? `<span class="rar r-common">Lv.${lv}</span>` : ''}</div>
          <div class="im-attr">${attrLine(it.eff)}</div>
          <div class="im-own">${owned ? (isOn ? '已装备 · 生效中' : '已拥有 · 在背包中') : '未拥有 · 购买后自动入包'}</div>
        </div>
        ${act}
      </div>`;
    }).join('');

    const note = `<div class="group-note">${esc(D.catById[cat].name)}：${esc(D.catById[cat].attrLabel)}。装备可随时更换，旧装备自动回到背包。</div>`;

    return `${note}<div class="item-list">${rendered}</div>`;
  }

  function catTone(cat) {
    return { rod: 'sun', bait: 'mint', hook: 'coral', line: 'lilac', float: 'sky' }[cat] || 'sky';
  }

  function shopScene(state) {
    const cards = D.SCENES.map((sc, i) => {
      const owned = state.scenesOwned.indexOf(sc.id) >= 0;
      const prev = D.SCENES[i - 1];
      const locked = !owned && prev && state.scenesOwned.indexOf(prev.id) < 0;
      const isOn = state.scene === sc.id;
      const act = isOn
        ? `<span class="act equipped">使用中</span>`
        : owned
          ? `<button class="act owned" data-act="switchScene" data-id="${sc.id}">进入钓场</button>`
          : locked
            ? `<span class="act locked">需先解锁 ${esc(prev.name)}</span>`
            : `<button class="act buy" data-act="buyScene" data-id="${sc.id}">${icon('sparkle')} ${fmt(sc.price)}</button>
               <span class="act buy" data-act="luckBuyScene" data-id="${sc.id}" style="cursor:pointer" title="放生渔运兑换">🎣${E.luckPrice(sc.price)}</span>`;
      return `<div class="scene-card ${isOn ? 'on' : ''}">
        <div class="scene-thumb" style="background:linear-gradient(180deg,${sc.palettes.day[0]},${sc.palettes.day[1]} 55%,${sc.palettes.day[2]})">
          ${isOn ? '<span class="st-now">当前钓场</span>' : ''}
          ${owned ? '' : `<span class="st-lock">${icon('lock')}</span>`}
          <span class="st-ic">${icon(sc.id === 'bay' ? 'palm' : 'scene')}</span>
        </div>
        <div class="scene-body">
          <div class="sb-nm">${esc(sc.name)} ${rarTag(sc.rarity)}</div>
          <div class="sb-attr">${esc(sc.desc)}</div>
          <div class="sb-act">${act}</div>
        </div>
      </div>`;
    }).join('');

    return `<div class="scene-grid">${cards}</div>
      <div class="group-note" style="padding-top:10px">场景决定鱼种池与稀有度倾向；时段与天气在钓场页右上角切换。</div>`;
  }

  /** 打窝饵商店：每竿消耗 1 枚；高档饵花魔力（主回收口），蚯蚓免费。 */
  function shopBait(state) {
    const rows = D.BAITS.map((b) => {
      const have = (state.baits && state.baits[b.id]) || 0;
      const on = state.baitId === b.id;
      const effTxt = Object.entries(b.eff || {})
        .map(([k, v]) => `${k === 'rare' ? '稀有' : k === 'epic' ? '史诗' : '传说'} +${Math.round(v * 100)}%`)
        .join(' · ') || '基础饵';
      const btn = b.price === 0
        ? `<span class="act ${on ? 'equipped' : 'owned'}">${on ? '使用中' : '免费'}</span>`
        : `<span class="act buy" data-act="selectBait" data-id="${b.id}" style="cursor:pointer">${on ? '使用中' : '挂上'}</span>
           <span class="act buy" data-act="buyBait" data-id="${b.id}" style="cursor:pointer">✦${fmt(b.price)}</span>`;
      return `<div class="item-card">
        <div class="ic-box tone-gold">${icon('bait')}</div>
        <div class="im"><div class="im-top"><span class="im-nm">${b.name}</span>${b.price === 0 ? '<span class="rar r-fine">免费</span>' : ''}</div>
        <div class="im-attr">${effTxt} · 每竿消耗 1 枚</div>
        <div class="im-own">${b.desc}${b.price > 0 ? ` · 库存 ${have}` : ''}</div></div>
        ${btn}
      </div>`;
    }).join('');
    return `<div class="item-list" style="padding:0 0 20px">${rows}</div>
      <div class="cap-sub" style="padding:0 16px 12px"><span>放生渔获可攒「渔运」（当前 ${state.luck || 0}）</span><span>蚯蚓打窝饵免费随手可得</span></div>`;
  }

  function shopBag(state) {
    const tiers = D.CONST.bagTiers.map((t, i) => {
      const cur = state.bag.tier;
      const done = i <= cur;
      const isNext = i === cur + 1;
      const locked = i > cur + 1;
      const act = done
        ? `<span class="act equipped">${i === cur ? '当前档位' : '已解锁'}</span>`
        : isNext
          ? `<button class="act buy" data-act="expandBag">${icon('sparkle')} ${fmt(t.price)}</button> <span class="act buy" data-act="luckExpandBag" style="cursor:pointer" title="放生渔运兑换">🎣${E.luckPrice(t.price)}</span>`
          : `<span class="act locked">需前档解锁</span>`;
      return `<div class="item-card">
        <div class="ic-box ${done ? 'tone-mint' : 'tone-sky'}">${icon('bag')}</div>
        <div class="im">
          <div class="im-top"><span class="im-nm">${t.cap} 格背包</span>${done ? '<span class="rar r-fine">已解锁</span>' : ''}</div>
          <div class="im-attr">${esc(t.label)}${t.skin ? ' · ' + esc(t.skin) : ''}</div>
          <div class="im-own">临时鱼缸容量 ${(i + 1) * D.CONST.aquariumCapPerTier} 尾 · 渔获与渔具共用容量</div>
        </div>
        ${act}
      </div>`;
    }).join('');

    const cap = E.bagCap(state);
    return `<div class="cap-card">
        <div class="cap-top"><span class="ct-t">当前容量</span><span class="ct-n">${E.bagUsed(state)} / ${cap}</span></div>
        <div class="cap-bar ${E.bagUsed(state) / cap > .9 ? 'warn' : ''}"><div style="width:${Math.min(100, E.bagUsed(state) / cap * 100)}%"></div></div>
        <div class="cap-sub"><span>渔获 <b>${state.bag.fish.length}</b></span><span>渔具 <b>${E.gearOwnedCount(state)}</b></span><span>扩容一次永久生效</span></div>
      </div>
      <div class="section-t"><span class="st-n">${icon('bag')}扩容档位</span><span class="st-r">按顺序解锁</span></div>
      <div class="item-list">${tiers}</div>`;
  }

  function shopAuto(state) {
    const owned = state.autoUnlocked;
    return `<div class="wide-card">
      <div class="wc-top">
        <div class="wc-ic tone-gold">${icon('bolt')}</div>
        <div class="wc-meta">
          <div class="wc-nm">自动垂钓${owned ? ' · 已解锁' : ''}</div>
          <div class="wc-sub">自动完成抛竿、起竿与收线；仍消耗体力，背包满时自动转入临时鱼缸。</div>
        </div>
      </div>
      <div class="wc-foot">
        <span class="wc-hint">${owned ? '在钓场点右侧「自动」即可开关' : '一次买断 · 永久生效'}</span>
        ${owned
          ? `<button class="cta cta-ghost cta-sm" data-act="toggleAuto">${state.auto ? '关闭自动' : '开启自动'}</button>`
          : `<button class="act buy" data-act="unlockAuto">${icon('sparkle')} ${fmt(D.CONST.autoUnlockPrice)}</button> <span class="act buy" data-act="luckUnlockAuto" style="cursor:pointer" title="放生渔运兑换">🎣${E.luckPrice(D.CONST.autoUnlockPrice)}</span>`}
      </div>
    </div>
    <div class="wide-card">
      <div class="wc-top">
        <div class="wc-ic tone-sky">${icon('clock')}</div>
        <div class="wc-meta">
          <div class="wc-nm">免费抛竿次数</div>
          <div class="wc-sub">签到、好友赠饵与饵箱会发放；优先于体力消耗。</div>
        </div>
        <span class="ct-n" style="font-size:15px;font-weight:900;color:var(--sky-deep)">${state.freeCasts}</span>
      </div>
    </div>`;
  }

  /* ---------------- 渔具页 ---------------- */
  function renderGear(state) {
    const stats = E.statsOf(state);
    const set = E.activeSet(state);
    const progress = E.setProgress(state);

    const summary = `<div class="grow-stats">
      <div class="gs-t"><span>当前生效属性汇总</span><span>${set ? '套装 · ' + esc(set.name) : '未激活套装'}</span></div>
      <div class="attr-row"><span class="ar-k">起竿窗口</span><span class="ar-v">${Math.round(stats.window)} ms</span></div>
      <div class="attr-row"><span class="ar-k">咬钩速度 / 中钩率</span><span class="ar-v up">+${pct(stats.biteSpeed)} / +${pct(stats.hookRate)}</span></div>
      <div class="attr-row"><span class="ar-k">稀有鱼 / 传说鱼概率</span><span class="ar-v up">+${pct(stats.rare)} / +${pct(stats.legend)}</span></div>
      <div class="attr-row"><span class="ar-k">断线阈值 / 收线效率</span><span class="ar-v">${Math.round(stats.limit)} 点 / ${Math.round(stats.pull * 100)}%</span></div>
      <div class="attr-row"><span class="ar-k">脱钩率 / 鱼群警觉</span><span class="ar-v up">-${pct(stats.escape)} / -${pct(stats.alert)}</span></div>
    </div>`;

    const slots = [
      { cat: 'scene', lb: '场景', ic: 'scene', id: state.scene, name: D.sceneById[state.scene].name, attr: D.sceneById[state.scene].desc, view: 'shop' },
      ...D.CATS.map((c) => {
        const it = D.itemById[state.equipped[c.id]] || {};
        const lv = E.itemLv(state, c.id, state.equipped[c.id]) || 1;
        return { cat: c.id, lb: c.name, ic: c.icon, id: it.id, name: it.name, attr: attrLine(it.eff), lv };
      })
    ].map((s) => {
      if (s.cat === 'scene') {
        return `<div class="slot-row">
          <div class="sr-ic tone-sky">${icon('scene')}</div>
          <div class="sr-meta"><div class="sr-k">场景</div><div class="sr-v">${esc(s.name)}</div><div class="sr-attr">${esc(s.attr)}</div></div>
          <div class="sr-act"><button class="mini-act keep" data-act="openView" data-view="shop" data-cat="scene">更换</button></div>
        </div>`;
      }
      return `<div class="slot-row ${s.lv > 1 ? 'on' : ''}">
        <div class="sr-ic ${tone(catTone(s.cat))}">${icon(s.ic)}</div>
        <div class="sr-meta">
          <div class="sr-k">${s.lb}${s.lv > 1 ? `<span class="rar r-epic">Lv.${s.lv}</span>` : ''}</div>
          <div class="sr-v">${esc(s.name)}</div>
          <div class="sr-attr">${esc(s.attr)}</div>
        </div>
        <div class="sr-act">
          <button class="mini-act keep" data-act="grow" data-cat="${s.cat}" data-id="${s.id}">养成</button>
          <button class="mini-act free" data-act="openItem" data-cat="${s.cat}" data-id="${s.id}">详情</button>
        </div>
      </div>`;
    }).join('');

    const heroSet = set
      ? `<div class="set-hero">
          <div class="sh-top">
            <div class="sh-ic">${icon('set')}</div>
            <div class="sh-meta">
              <div class="sh-name">${esc(set.name)} ${rarTag(set.rarity)}</div>
              <div class="sh-sub">六件齐套 · ${esc(D.sceneById[set.scene].name)} · 全套属性已叠加</div>
            </div>
            <span class="sh-state">已激活</span>
          </div>
          <div class="set-bonuses">${set.bonuses.map((b) => `<div class="set-bonus">${icon('check')}${esc(b.label)}</div>`).join('')}</div>
        </div>`
      : `<div class="set-hero" style="background:linear-gradient(135deg,#8e9cb8,#6a7a98)">
          <div class="sh-top">
            <div class="sh-ic">${icon('set')}</div>
            <div class="sh-meta">
              <div class="sh-name">暂未激活套装</div>
              <div class="sh-sub">同一场景集齐 5 件同套渔具即可获得齐套加成</div>
            </div>
            <span class="sh-state off">未激活</span>
          </div>
          <div class="set-bonuses"><div class="set-bonus">${icon('arrow')}点下方套装卡查看所需件数与 buff</div></div>
        </div>`;

    const setSlots = (set || D.SETS[0]).parts;
    const partKeys = ['rod', 'bait', 'hook', 'line', 'float'];
    const setSlotHtml = partKeys.map((cat) => {
      const wantId = set ? set.parts[cat] : null;
      const it = wantId ? D.itemById[wantId] : null;
      const has = wantId ? E.isOwned(state, cat, wantId) : false;
      return `<div class="set-slot ${has ? '' : 'miss'}">
        <div class="ss-ic ${has ? tone(catTone(cat)) : ''}">${icon(D.catById[cat].icon)}</div>
        <div class="ss-lb">${has ? esc(it.name).slice(0, 4) : '缺' + D.catById[cat].name}</div>
      </div>`;
    }).join('');

    const setCards = progress.map(({ set: s, got, total, equipped }) => {
      const allOwned = got >= total;
      const prevNeed = s.needPrev ? D.setById[s.needPrev] : null;
      const prevOk = !prevNeed || E.setProgress(state).find((p) => p.set.id === s.needPrev).equipped;
      return `<div class="set-card ${equipped ? 'active' : ''}">
        <div class="sc-top">
          <div class="sc-ic ${tone(sceneTone(s.scene))}">${icon('set')}</div>
          <div class="sc-meta">
            <div class="sc-name">${esc(s.name)} ${rarTag(s.rarity)}</div>
            <div class="sc-pri">${esc(D.sceneById[s.scene].name)} · 六件齐套${s.needPrev ? ' · 需先激活' + esc(prevNeed.name) : ''}</div>
          </div>
          <div class="sc-count">${got}/${total}</div>
        </div>
        <div class="sc-bonuses">${s.bonuses.map((b) => `<div class="sc-bonus">${icon('check')}${esc(b.label)}</div>`).join('')}</div>
        <div class="sc-foot">
          <span class="sc-hint">${equipped ? '套装生效中' : allOwned ? (prevOk ? '件数已齐 · 可一键装配' : '需先激活前置套装') : '还差 ' + (total - got) + ' 件'}</span>
          ${equipped
            ? `<span class="act equipped">已激活</span>`
            : allOwned && prevOk
              ? `<button class="act buy" data-act="equipSet" data-id="${s.id}">一键装配</button>`
              : `<button class="act locked" data-act="openSetParts" data-id="${s.id}">查看清单</button>`}
        </div>
      </div>`;
    }).join('');

    return `<div class="view-inner">
      <div class="page-head">
        <div><div class="ph-t">渔具 · 套装与养成</div><div class="ph-s">六类装备可更换、可升级；同套齐备触发额外 buff</div></div>
      </div>
      ${summary}
      <div class="section-t"><span class="st-n">${icon('gear')}已装备</span><span class="st-r">点「养成」升阶</span></div>
      <div class="slot-list">${slots}</div>
      <div class="section-t"><span class="st-n">${icon('set')}套装加成</span><span class="st-r">6/6 激活</span></div>
      ${heroSet}
      <div class="set-slots">${setSlotHtml}</div>
      <div class="set-list">${setCards}</div>
    </div>`;
  }

  /* ---------------- 背包 ---------------- */
  function renderBag(state) {
    const cap = E.bagCap(state);
    const used = E.bagUsed(state);
    const pctUsed = Math.min(100, used / cap * 100);
    const fish = state.bag.fish;
    const cells = fish.map((f) => fishCell(f)).join('');
    const emptyCount = Math.max(0, cap - used);
    const placeholders = Math.min(emptyCount, 8);
    const emptyCells = Array.from({ length: placeholders }, () => '<div class="fish-cell empty"><span class="fc-q">空</span></div>').join('');
    const moreCell = emptyCount > placeholders ? `<div class="fish-cell more"><span class="fc-nm">+${emptyCount - placeholders} 空位</span></div>` : '';

    const next = D.CONST.bagTiers[state.bag.tier + 1];
    const expand = next
      ? `<div class="wide-card">
          <div class="wc-top">
            <div class="wc-ic tone-gold">${icon('bag')}</div>
            <div class="wc-meta"><div class="wc-nm">${next.label}</div><div class="wc-sub">扩容后背包容量 ${next.cap} 格，临时鱼缸同步扩至 ${(state.bag.tier + 2) * D.CONST.aquariumCapPerTier} 尾</div></div>
          </div>
          <div class="wc-foot">
            <span class="wc-hint">当前 ${cap} 格 · 已用 ${used}</span>
            <button class="act buy" data-act="expandBag">${icon('sparkle')} ${fmt(next.price)}</button>
          </div>
        </div>`
      : `<div class="wide-card"><div class="wc-top"><div class="wc-ic tone-mint">${icon('check')}</div><div class="wc-meta"><div class="wc-nm">背包已满阶 · 120 格</div><div class="wc-sub">渔获与渔具共用容量，已解锁专属背包皮肤</div></div></div></div>`;

    return `<div class="view-inner">
      <div class="page-head">
        <div><div class="ph-t">背包 · 渔获</div><div class="ph-s">渔获与渔具共用容量；满包时渔获转入临时鱼缸</div></div>
      </div>
      <div class="cap-card">
        <div class="cap-top"><span class="ct-t">容量占用</span><span class="ct-n">${used} / ${cap}</span></div>
        <div class="cap-bar ${pctUsed > 90 ? 'warn' : ''}"><div style="width:${pctUsed}%"></div></div>
        <div class="cap-sub"><span>渔获 <b>${fish.length}</b></span><span>渔具 <b>${E.gearOwnedCount(state)}</b></span><span>空位 <b>${E.bagFree(state)}</b></span></div>
      </div>
      <div class="section-t"><span class="st-n">${icon('fish')}渔获陈列</span><span class="st-r">点击查看 / 放生</span></div>
      ${fish.length
        ? `<div class="fish-grid">${cells}${emptyCells}${moreCell}</div>`
        : `<div class="empty-state"><div class="es-ic">${icon('fish')}</div><div class="es-t">还没有渔获</div><div class="es-s">去钓场抛一竿，钓获会自动进包<br>背包满后转入临时鱼缸暂存</div><button class="cta cta-primary cta-sm" style="margin:14px auto 0" data-act="gotoPond">去抛竿</button></div>`}
      <div class="section-t"><span class="st-n">${icon('aqua')}临时鱼缸</span><span class="st-r">${state.aquarium.length}/${E.aquariumCap(state)}</span></div>
      <div class="wide-card">
        <div class="wc-top">
          <div class="wc-ic tone-lilac">${icon('aqua')}</div>
          <div class="wc-meta"><div class="wc-nm">满包暂存区</div><div class="wc-sub">${state.aquarium.length ? `有 ${state.aquarium.length} 尾暂存，及时取回或放生` : '当前无暂存渔获，容量充足时渔获直接入包'}</div></div>
          <button class="act owned" data-act="openView" data-view="aquarium">查看</button>
        </div>
      </div>
      ${expand}
    </div>`;
  }

  function fishCell(f) {
    const r = D.RARITY[f.rarity] || D.RARITY.common;
    return `<button class="fish-cell" data-act="openFish" data-uid="${f.uid}">
      <span class="fc-ic" style="background:linear-gradient(135deg,${r.color}cc,${r.color})">${icon('fish')}</span>
      <span class="fc-nm">${esc(f.name)}</span>
      <span class="fc-mg">✦${f.magic}</span>
      <span class="fc-dot" style="background:${r.color}"></span>
    </button>`;
  }

  /* ---------------- 图鉴 ---------------- */
  function renderAlbum(state) {
    const collected = Object.keys(state.album).length;
    const total = D.FISH.length;
    const groups = E.albumView(state);

    const groupsHtml = groups.map((g) => {
      const cells = g.list.map(({ fish, got }) => {
        const r = D.RARITY[fish.rarity];
        if (!got) {
          return `<button class="album-cell locked" data-act="openFish" data-id="${fish.id}">
            <span class="ac-ic">${icon('fish')}</span><span class="ac-nm">未知</span>
            <span class="ac-lock">${icon('lock')}</span></button>`;
        }
        const rec = state.album[fish.id];
        return `<button class="album-cell" data-act="openFish" data-id="${fish.id}">
          <span class="ac-ic" style="color:${r.color}">${icon('fish')}</span>
          <span class="ac-nm">${esc(fish.name)}</span>
          <span class="ac-cnt">×${rec.count}</span></button>`;
      }).join('');
      return `<div class="group-head">
          <span class="gh-ic tone-${sceneTone(g.sceneId)}">${icon(condIcon(g.cond))}</span>
          <span class="gh-t">${esc(g.sceneName)} · ${g.condName}</span>
          <span class="gh-n">${g.got}/${g.total}${g.unlocked ? '' : ' · 未解锁'}</span>
        </div>
        <div class="album-grid">${cells}</div>`;
    }).join('');

    return `<div class="view-inner">
      <div class="page-head">
        <div><div class="ph-t">鱼类图鉴</div><div class="ph-s">按场景 × 时段分区 · 点击剪影查看钓获条件</div></div>
      </div>
      <div class="album-summary">
        <div class="as-top"><span class="as-t">收录进度</span><span class="as-n">${collected} / ${total}</span></div>
        <div class="cap-bar gold"><div style="width:${collected / total * 100}%"></div></div>
        <div class="cap-sub"><span>稀有度图例</span><span style="color:#8a9ab0">普通</span><span style="color:#4a7ab0">精良</span><span style="color:#6a7ab8">稀有</span><span style="color:#b08a4a">史诗</span><span style="color:#b86868">传说</span></div>
      </div>
      ${groupsHtml}
      <div style="height:14px"></div>
    </div>`;
  }

  /* ---------------- 订单 ---------------- */
  function renderOrders(state) {
    E.ensureOrders(state, Date.now());
    const list = state.orders.list;
    const rows = list.map((o, i) => {
      const f = D.fishById[o.id] || {};
      const done = o.got >= o.need;
      const act = o.claimed
        ? `<span class="act equipped">已领取</span>`
        : done
          ? `<button class="act claim" data-act="claimOrder" data-idx="${i}">领取</button>`
          : `<span class="act wait">${o.got}/${o.need}</span>`;
      return `<div class="goal-row">
        <div class="gr-ic tone-${sceneTone(f.scene || 'bay')}">${icon('fish')}</div>
        <div class="gr-meta">
          <div class="gr-nm">${esc(f.name || '?')} ${rarTag(f.rarity || 'common')}</div>
          <div class="gr-pri">${esc(D.sceneById[f.scene] ? D.sceneById[f.scene].name : '')} · ${condName(f.cond)} · 体型 ${f.size ? f.size[0] + '–' + f.size[1] + 'cm' : '—'}</div>
          <div class="gr-bar"><div style="width:${Math.min(100, o.got / o.need * 100)}%"></div></div>
        </div>
        <div style="text-align:right;display:flex;flex-direction:column;gap:5px;align-items:flex-end">
          <span class="reward-chip">${icon('sparkle')}${fmt(o.reward)}</span>
          ${act}
        </div>
      </div>`;
    }).join('');

    const claimed = list.filter((o) => o.claimed).length;
    const bonusReady = E.allOrdersDone(state) && !state.orders.bonusClaimed;
    const bonus = `<div class="cap-card" style="background:linear-gradient(160deg,#fff,#f8f4e8);box-shadow:inset 0 0 0 1px var(--gold)">
        <div class="cap-top"><span class="ct-t">完成全部订单</span><span class="ct-n">额外 ✦ ${D.CONST.orderAllBonus}</span></div>
        <div class="cap-bar gold"><div style="width:${claimed / list.length * 100}%"></div></div>
        <div class="cap-sub"><span>已完成 <b>${claimed}</b> 单</span><span>今日累计可得 <b>✦ ${fmt(list.reduce((a, o) => a + o.reward, 0) + D.CONST.orderAllBonus)}</b></span>
        <span style="margin-left:auto">${state.orders.bonusClaimed ? '<span style="color:var(--success);font-weight:900">已领取</span>' : bonusReady ? `<button class="mini-act gift" data-act="claimOrderBonus">领取额外奖励</button>` : '全部完成后可领'}</span></div>
      </div>`;

    return `<div class="view-inner">
      <div class="page-head">
        <div><div class="ph-t">每日订单</div><div class="ph-s">每日 0 点重置 · 目标鱼种按已解锁钓场生成</div></div>
        <div class="ph-act"><span class="magic-pill">${icon('clock')}<b id="orderLeft">—</b></span></div>
      </div>
      <div class="order-big">
        <div class="ob-head"><span class="ob-t">今日委托</span><span class="ob-refresh">${esc(state.orders.day)} · 次日重置</span></div>
        ${rows}
      </div>
      ${bonus}
      <div class="group-note" style="padding:10px 16px">钓获订单目标鱼即自动计入进度，无需手动提交；领取奖励后进度锁定。</div>
    </div>`;
  }

  /* ---------------- 体力 / 签到 ---------------- */
  function renderStamina(state) {
    const C = D.CONST;
    const pips = Array.from({ length: C.staminaMax }, (_, i) => `<i class="${i < state.stamina.v ? 'on' : ''}"></i>`).join('');
    const nextIn = E.staminaNextIn(state, Date.now());
    const nextTxt = state.stamina.v >= C.staminaMax ? '体力已满' : '下一点 ' + mmss(nextIn);
    const full = state.stamina.v >= C.staminaMax;

    const sign = E.signState(state, Date.now());
    const doneCount = sign.claimedToday ? (sign.idx === 0 ? C.signInRewards.length : sign.idx) : sign.idx;
    const days = C.signInRewards.map((rw, i) => {
      const doneBefore = i < doneCount;
      const isToday = !sign.claimedToday && i === sign.idx;
      return `<div class="sign-day ${doneBefore ? 'done' : isToday ? 'today' : ''}">
        <div class="sd-d">D${rw.day}</div>
        <div class="sd-rw">${rw.type === 'magic' || rw.type === 'chest' ? icon('sparkle') : rw.type === 'bait' ? icon('bait') : icon('bolt')}</div>
        <div class="sd-rw">${rw.type === 'stamina' ? '体力+' + rw.value : rw.label}</div>
      </div>`;
    }).join('');

    return `<div class="view-inner">
      <div class="page-head"><div><div class="ph-t">体力与签到</div><div class="ph-s">每 ${C.staminaRecoverMs / 60000} 分钟恢复 1 点 · 上限 ${C.staminaMax} 点 · 离线同样累积</div></div></div>
      <div class="stamina-card">
        <div class="st-ic">${icon('bolt')}</div>
        <div class="st-meta">
          <div class="st-nm">魔力体力</div>
          <div class="st-val">${state.stamina.v} / ${C.staminaMax} · ${nextTxt}${state.freeCasts ? ' · 免费抛竿 ' + state.freeCasts : ''}</div>
          <div class="st-grid">${pips}</div>
        </div>
        <button class="st-buy" data-act="refill" ${full ? 'disabled' : ''}>${full ? '已满' : icon('sparkle') + ' ' + C.staminaRefillCost + '<br>补满'}</button>
      </div>
      <div class="sign-wrap">
        <div class="sw-t">七日签到 <span>${sign.claimedToday ? '今日已签 · 明日继续' : '今日可签'}</span></div>
        <div class="sign-grid">${days}</div>
        <div class="cap-sub" style="margin-top:10px"><span>第 7 日传说饵箱：✦400 + 鱼饵×6</span></div>
        <button class="cta cta-gold" style="margin:12px 0 0;width:100%" data-act="sign" ${sign.claimedToday ? 'disabled' : ''}>${sign.claimedToday ? '今日已签到' : '领取今日签到奖励'}</button>
      </div>
      <div class="wide-card">
        <div class="wc-top">
          <div class="wc-ic tone-sky">${icon('rod')}</div>
          <div class="wc-meta"><div class="wc-nm">抛竿消耗</div><div class="wc-sub">每次抛竿消耗 ${C.castStaminaCost} 点体力；拥有免费次数时优先消耗免费次数。体力为 0 时无法抛竿，可用 ✦${C.staminaRefillCost} 补满。</div></div>
        </div>
      </div>
    </div>`;
  }

  function mmss(ms) {
    const s = Math.max(0, Math.ceil(ms / 1000));
    const h = Math.floor(s / 3600);
    const m = Math.floor((s % 3600) / 60);
    const sec = s % 60;
    if (h > 0) return h + ':' + String(m).padStart(2, '0') + ':' + String(sec).padStart(2, '0');
    return m + ':' + String(sec).padStart(2, '0');
  }

  /* ---------------- 好友 ---------------- */
  function renderFriends(state) {
    const fs = E.friendState(state, Date.now());
    const view = E.friendView(state, Date.now());
    const rows = view.map((f) => {
      const sent = fs.sent >= D.CONST.giftDailyLimit;
      return `<div class="friend-row">
        <div class="fr-avatar ${tone(f.tone)}">${esc(f.tag)}${f.online ? '<span class="online"></span>' : ''}</div>
        <div class="fr-meta">
          <div class="fr-nm">${esc(f.name)}<span class="fr-tag">${esc(D.sceneById[f.scene].name)}</span></div>
          <div class="fr-pri">${f.online ? '在线 · ' : '离线 · '}${esc(f.rod)} · 鱼塘 ${f.pond.map((p) => esc(p.name)).join(' / ')}</div>
        </div>
        <div class="fr-acts">
          <button class="mini-act free" data-act="visitFriend" data-id="${f.id}">拜访</button>
          <button class="mini-act gift" data-act="gift" data-id="${f.id}" ${sent ? 'disabled' : ''}>赠饵</button>
        </div>
      </div>`;
    }).join('');

    return `<div class="view-inner">
      <div class="page-head"><div><div class="ph-t">好友鱼塘</div><div class="ph-s">每日可赠饵 ${D.CONST.giftDailyLimit} 次 · 收到的鱼饵转为免费抛竿</div></div></div>
      <div class="friend-bar">
        <div class="fb-box"><div class="fb-n">${fs.received.length}</div><div class="fb-k">待收鱼饵</div></div>
        <div class="fb-box"><div class="fb-n">${D.CONST.giftDailyLimit - fs.sent}</div><div class="fb-k">今日可赠</div></div>
        <div class="fb-box"><div class="fb-n">${state.freeCasts}</div><div class="fb-k">免费抛竿</div></div>
      </div>
      <div class="wide-card" style="display:flex;align-items:center;gap:10px">
        <div class="wc-meta" style="flex:1"><div class="wc-nm">收下好友赠礼</div><div class="wc-sub">${fs.received.length ? `有 ${fs.received.length} 份鱼饵待收，领取后转为免费抛竿` : '暂无可收鱼饵，明天再来看看'}</div></div>
        <button class="act ${fs.received.length ? 'buy' : 'locked'}" data-act="receiveGifts" ${fs.received.length ? '' : 'disabled'}>一键收取</button>
      </div>
      <div class="section-t"><span class="st-n">${icon('friend')}好友列表</span><span class="st-r">${view.filter((f) => f.online).length} 位在线</span></div>
      <div class="friend-list">${rows}</div>
      <div class="group-note" style="padding:0 16px 16px">拜访好友鱼塘可查看其展示鱼种与当前装备；赠饵会消耗 1 次每日额度，好友上线后可回赠。</div>
    </div>`;
  }

  /* ---------------- 成就 ---------------- */
  function renderAch(state) {
    const views = E.achView(state);
    const done = views.filter((v) => v.done).length;
    const claimable = views.filter((v) => v.claimable).length;
    const badges = views.map((v) => {
      if (!v.done) {
        return `<div class="ach-badge locked"><div class="ab-ic">${icon(v.def.icon)}</div><div class="ab-nm">${esc(v.def.name)}</div></div>`;
      }
      const t = v.def.group === 'wealth' ? 'gold' : v.def.group === 'social' ? 'lilac' : v.def.group === 'daily' ? 'sky' : 'mint';
      return `<div class="ach-badge"><div class="ab-ic ${tone(t)}">${icon(v.def.icon)}</div><div class="ab-nm">${esc(v.def.name)}</div></div>`;
    }).join('');

    const groups = [
      { id: 'fishing', name: '垂钓' },
      { id: 'album', name: '图鉴' },
      { id: 'explore', name: '探索' },
      { id: 'grow', name: '养成' },
      { id: 'daily', name: '日常' },
      { id: 'social', name: '社交' },
      { id: 'wealth', name: '财力' },
    ];
    const rows = groups.map((g) => {
      const items = views.filter((v) => v.def.group === g.id);
      if (!items.length) return '';
      const list = items.map((v) => `<div class="ach-row ${v.done ? 'done' : ''}">
        <div class="ar-ic ${v.done ? '' : tone(g.id === 'wealth' ? 'gold' : 'sky')}">${icon(v.def.icon)}</div>
        <div class="ar-meta">
          <div class="ar-nm">${esc(v.def.name)}${v.claimed ? '<span class="rar r-fine">已领取</span>' : ''}</div>
          <div class="ar-prog">${esc(v.def.desc)} · ${Math.min(v.prog, v.def.target)}/${v.def.target}${v.def.metric === 'magicEarned' ? ' 魔力' : ''}</div>
          <div class="ar-bar"><div style="width:${Math.min(100, v.prog / v.def.target * 100)}%"></div></div>
        </div>
        <div class="ar-rw">✦${v.def.reward}${v.claimable ? `<br><button class="mini-act gift" style="margin-top:5px" data-act="claimAch" data-id="${v.def.id}">领取</button>` : ''}</div>
      </div>`).join('');
      return `<div class="section-t"><span class="st-n">${icon('medal')}${g.name}成就</span><span class="st-r">${items.filter((v) => v.done).length}/${items.length}</span></div>
        <div class="ach-rows">${list}</div>`;
    }).join('');

    return `<div class="view-inner">
      <div class="page-head"><div><div class="ph-t">成就徽章墙</div><div class="ph-s">达成条件点亮徽章 · 奖励额度仅计入渔获估值（不发魔力）</div></div></div>
      <div class="ach-summary">
        <div class="as-top"><span class="as-t">徽章进度</span><span class="as-n">${done} / ${views.length}</span></div>
        <div class="cap-bar gold"><div style="width:${done / views.length * 100}%"></div></div>
        <div class="cap-sub"><span>已解锁 <b>${done}</b> 枚</span><span>可领取 <b>${claimable}</b> 枚</span><span>累计可领 ✦<b>${fmt(views.filter((v) => !v.claimed).reduce((a, v) => a + v.def.reward, 0))}</b></span></div>
      </div>
      <div class="ach-grid">${badges}</div>
      ${rows}
      <div style="height:12px"></div>
    </div>`;
  }

  /* ---------------- 排行 ---------------- */
  function renderRank(state) {
    // 站点模式：main.js 已把服务端真实赛季榜写入 G.__FT_RANK；离线/单机才退回 NPC 模拟榜。
    const board = (G.__FT_RANK && G.__FT_RANK.rows && G.__FT_RANK.rows.length)
      ? G.__FT_RANK : E.rankBoard(state, Date.now());
    const rows = board.rows.map((r) => `<div class="rank-row ${r.me ? 'me' : ''}">
        <div class="rr-no ${r.rank === 1 ? 'top1' : r.rank === 2 ? 'top2' : r.rank === 3 ? 'top3' : ''}">${r.rank}</div>
        <div class="rr-avatar ${tone(r.tone)}">${esc(r.tag)}</div>
        <div class="rr-meta">
          <div class="rr-nm">${esc(r.name)}${r.me ? '<span class="me-tag">我</span>' : ''}</div>
          <div class="rr-pri">本赛季渔获 ${fmt(r.catches)} 尾</div>
        </div>
        <div class="rr-val"><div class="rr-v">${r.rank <= 8 ? '✦' + fmt(r.reward) : '—'}</div><div class="rr-rw">${r.rank <= 3 ? '名次奖励' : '赛季结算'}</div></div>
      </div>`).join('');

    return `<div class="view-inner">
      <div class="page-head"><div><div class="ph-t">渔场排行榜</div><div class="ph-s">赛季 ${D.CONST.seasonDays} 天 · 按累计渔获尾数排名 · 名次奖励不发魔力</div></div></div>
      <div class="rank-summary">
        <div><div class="rs-t">第 ${board.info.ordinal} 赛季 · 剩 ${board.info.leftDays} 天</div><div class="rs-s">我的渔获 ${board.season.catches} 尾 · 距上一名 ${board.me.rank > 1 ? fmt(board.rows[board.me.rank - 2].catches - board.me.catches + 1) + ' 尾' : '已是榜首'}</div></div>
        <div class="rs-me"><div class="rs-r">#${board.me.rank}</div><div class="rs-k">当前名次</div></div>
      </div>
      <div class="rank-list">${rows}</div>
      <div class="group-note" style="padding:0 16px 16px">名次奖励：第 1 名 ✦500，第 2 ✦300，第 3 ✦200（额度展示）；赛季切换时渔获清零，奖励发放以站点公告为准。</div>
    </div>`;
  }

  /* ---------------- 临时鱼缸 ---------------- */
  function renderAquarium(state) {
    const cap = E.aquariumCap(state);
    const rows = state.aquarium.map((f) => {
      const r = D.RARITY[f.rarity] || D.RARITY.common;
      return `<div class="aqua-row">
        <div class="ar-ic" style="background:linear-gradient(135deg,${r.color}cc,${r.color})">${icon('fish')}</div>
        <div class="ar-meta">
          <div class="ar-nm">${esc(f.name)} ${rarTag(f.rarity)}</div>
          <div class="ar-pri">${esc(D.sceneById[f.scene] ? D.sceneById[f.scene].name : '')} · ${f.size}cm · ${f.weight}kg · ✦${f.magic}</div>
        </div>
        <div class="ar-acts">
          <button class="mini-act keep" data-act="takeFish" data-uid="${f.uid}">入包</button>
          <button class="mini-act free" data-act="releaseFish" data-uid="${f.uid}">放生</button>
        </div>
      </div>`;
    }).join('');

    return `<div class="view-inner">
      <div class="page-head"><div><div class="ph-t">临时鱼缸</div><div class="ph-s">背包满时的兜底暂存区 · 容量随背包档位提升</div></div>
        <div class="ph-act"><span class="magic-pill">${icon('aqua')}<b>${state.aquarium.length}/${cap}</b></span></div></div>
      <div class="aqua-note">${icon('aqua')}<span>渔获满包后会先进入临时鱼缸，不丢失；取回受背包空位限制，放生可立即腾出鱼缸位置（渔获不返魔力，估值已计入图鉴）。</span></div>
      ${state.aquarium.length
        ? `<div class="sheet-actions" style="margin:10px 16px 0"><button class="cta cta-primary cta-sm" style="flex:1" data-act="takeAll">一键取回</button><button class="cta cta-ghost cta-sm" style="flex:1" data-act="releaseAll">全部放生</button></div>
           <div class="aqua-list">${rows}</div>`
        : `<div class="empty-state"><div class="es-ic">${icon('aqua')}</div><div class="es-t">鱼缸是空的</div><div class="es-s">背包满时渔获会自动暂存到这里<br>扩容背包或取回渔获即可腾出空间</div></div>`}
      <div style="height:14px"></div>
    </div>`;
  }

  /* ================= 弹窗 ================= */

  function sheetWrap(inner) {
    return `<div class="mask" data-act="closeSheet"></div><div class="sheet-wrap"><div class="sheet" id="sheetBody">
      <button class="mc-close" data-act="closeSheet">${icon('x')}</button>${inner}</div></div>`;
  }

  function itemSheet(state, cat, id) {
    const it = D.itemById[id];
    if (!it) return '';
    const owned = E.isOwned(state, cat, id);
    const equipped = state.equipped[cat] === id;
    const curId = state.equipped[cat];
    const cur = D.itemById[curId] || { name: '未装备', eff: {} };
    const lv = E.itemLv(state, cat, id) || 1;

    const cmpRows = (() => {
      const keys = Array.from(new Set([...Object.keys(cur.eff || {}), ...Object.keys(it.eff || {})]));
      if (!keys.length) return `<div class="attr-row"><span class="ar-k">属性</span><span class="ar-v">标准款 · 无加成</span></div>`;
      return keys.map((k) => {
        const a = (cur.eff || {})[k] || 0;
        const b = (it.eff || {})[k] || 0;
        const meta = EFF[k] || { k, f: (v) => String(v) };
        const delta = b - a;
        const cls = delta > 0 ? 'up' : delta < 0 ? 'down' : '';
        const deltaTxt = (delta !== 0 && a > 0) ? `<span class="ar-d" style="color:${delta > 0 ? 'var(--success)' : 'var(--danger)'}"> (${delta > 0 ? '+' : '-'}${meta.f(Math.abs(delta)).replace(/^\+/, '')})</span>` : '';
        return `<div class="attr-row"><span class="ar-k">${meta.k}</span><span class="ar-v ${cls}">${meta.f(b)}${deltaTxt}</span></div>`;
      }).join('');
    })();

    const actions = equipped
      ? `<span class="act equipped" style="flex:1">使用中</span>`
      : owned
        ? `<button class="cta cta-primary" data-act="equip" data-cat="${cat}" data-id="${id}">装上使用</button>`
        : `<button class="cta cta-gold" data-act="buy" data-cat="${cat}" data-id="${id}">${icon('sparkle')} ${fmt(it.price)} · 购买并装备</button>
           <button class="cta cta-ghost" data-act="luckBuyItem" data-cat="${cat}" data-id="${id}">🎣 ${E.luckPrice(it.price)} 渔运解锁（仅持有）</button>`;

    return sheetWrap(`
      <div class="modal-hero">
        <div class="mh-box ${tone(catTone(cat))}">${icon(D.catById[cat].icon)}</div>
        <div>
          <div class="mh-nm">${esc(it.name)} ${rarTag(it.rarity)}</div>
          <div class="mh-sub">${D.catById[cat].name} · ${esc(it.desc)}${owned ? ' · 已拥有 Lv.' + lv : ''}</div>
        </div>
      </div>
      <div class="cmp-duo" style="margin-bottom:10px">
        <div class="cmp-card cur">
          <span class="cc-tag">当前</span>
          <div class="cc-ic ${tone(catTone(cat))}" style="opacity:.85">${icon(D.catById[cat].icon)}</div>
          <div class="cc-nm">${esc(cur.name)}</div>
          <div class="cc-sub">Lv.${E.itemLv(state, cat, curId) || 1}</div>
        </div>
        <div class="cmp-card win">
          <span class="cc-tag">候选</span>
          <div class="cc-ic ${tone(catTone(cat))}">${icon(D.catById[cat].icon)}</div>
          <div class="cc-nm">${esc(it.name)}</div>
          <div class="cc-sub">${owned ? 'Lv.' + lv : '未拥有'}</div>
        </div>
      </div>
      <div class="attr-list">
        <div class="al-t">属性对照（右列为候选）</div>
        ${cmpRows}
      </div>
      <div class="compare">购买后原装备自动回到<b>背包</b>，可随时换回；装备属性立即计入钓场判定。</div>
      <div class="sheet-actions">${actions}</div>
    `);
  }

  function growSheet(state, cat, id) {
    const it = D.itemById[id];
    if (!it) return '';
    const lv = E.itemLv(state, cat, id) || 1;
    const prev = E.upgradePreview(state, cat, id);
    const max = D.CONST.upgradeMaxLv;
    const stars = Array.from({ length: max }, (_, i) => `<span class="${i < lv ? '' : 'off'}">${icon('star', i < lv ? '' : 'off')}</span>`).join('');
    const rows = Object.keys(it.eff || {}).map((k) => {
      const meta = EFF[k] || { k, f: (v) => String(v) };
      const a = prev.cur[k] || 0;
      const b = prev.next ? prev.next[k] : a;
      return `<div class="gstat"><span class="gs-v cur">${meta.f(a)}</span><span class="gs-k">${meta.k}</span><span class="gs-v next">${meta.f(b)}</span></div>`;
    }).join('') || `<div class="gstat"><span class="gs-v cur">—</span><span class="gs-k">标准款无加成</span><span class="gs-v next">—</span></div>`;

    const nextOther = D.ITEMS[cat].filter((x) => x.id !== id && E.isOwned(state, cat, x.id));
    return sheetWrap(`
      <div class="grow-hero" style="margin:0 0 10px">
        <div class="gh-box ${tone(catTone(cat))}">${icon(D.catById[cat].icon)}</div>
        <div class="gh-meta">
          <div class="gh-nm">${esc(it.name)} <span class="gh-lv">Lv.${lv} / ${max}</span></div>
          <div class="gh-sub">${D.catById[cat].name} · 升级不掉级、不碎装，逐级永久生效</div>
          <div class="gh-stars">${stars}</div>
        </div>
      </div>
      <div class="grow-stats" style="margin:0 0 10px">
        <div class="gs-t"><span>当前（Lv.${lv}）</span><span>升级后（Lv.${Math.min(max, lv + 1)}）</span></div>
        ${rows}
      </div>
      ${lv >= max
        ? `<div class="grow-next" style="margin:0 0 10px"><div class="gn-ic">${icon('check')}</div><div class="gn-meta"><div class="gn-nm">已达满级 Lv.${max}</div><div class="gn-pri">该装备已解锁全部成长收益</div></div></div>
           <div class="sheet-actions"><span class="cta cta-ghost" style="flex:1;pointer-events:none">满级</span></div>`
        : `<div class="grow-next" style="margin:0 0 10px"><div class="gn-ic">${icon('sparkle')}</div><div class="gn-meta"><div class="gn-nm">升级至 Lv.${lv + 1}</div><div class="gn-pri">消耗 ✦${fmt(prev.cost)} · 等级收益随装备原价提升</div></div></div>
           <div class="sheet-actions">
             <button class="cta cta-gold" data-act="upgrade" data-cat="${cat}" data-id="${id}" ${state.magic < prev.cost ? 'disabled' : ''}>${state.magic < prev.cost ? '魔力不足' : icon('sparkle') + ' ' + fmt(prev.cost) + ' 升级'}</button>
           </div>`}
      <div class="compare">同槽位其他已拥有装备：${nextOther.length ? nextOther.map((x) => esc(x.name) + ' Lv.' + E.itemLv(state, cat, x.id)).join(' · ') : '暂无'}。升级只作用于当前这件装备。</div>
    `);
  }

  function catchSheet(state, payload) {
    const list = payload.items;
    if (list.length === 1) {
      const it = list[0];
      const r = D.RARITY[it.rarity] || D.RARITY.common;
      const dest = it.dest === 'aquarium' ? '临时鱼缸' : it.dest === 'lost' ? '未能入包' : '背包';
      return sheetWrap(`
        <div class="catch-hero" style="background:linear-gradient(160deg,${r.color}bb,${r.color})">${icon('fish')}${it.isNew ? '<span class="ch-new">新鱼种 · 图鉴收录</span>' : ''}</div>
        <div class="catch-title">${esc(it.name)} ${rarTag(it.rarity)}</div>
        <div class="catch-sub">${esc(D.sceneById[state.scene].name)} · ${condName(state.cond)} · 体型 ${it.size}cm · ${it.weight}kg</div>
        <div class="reward-list">
          <div class="reward-row"><div class="rw-ic tone-gold">${icon('sparkle')}</div><div class="rw-meta"><div class="rw-k">渔获估值</div><div class="rw-v">计入图鉴（不返魔力）</div></div><div class="rw-n">+${fmt(it.magic)}</div></div>
          <div class="reward-row"><div class="rw-ic tone-sky">${icon('bag')}</div><div class="rw-meta"><div class="rw-k">渔获去向</div><div class="rw-v">${dest} · ${E.bagUsed(state)}/${E.bagCap(state)}</div></div><div class="rw-n" style="color:var(--sky-deep)">${it.dest === 'aquarium' ? '已暂存' : '已入包'}</div></div>
          <div class="reward-row"><div class="rw-ic tone-lilac">${icon('album')}</div><div class="rw-meta"><div class="rw-k">图鉴进度</div><div class="rw-v">${it.isNew ? '首次收录该鱼种' : '已收录 · 累计 ' + (state.album[it.id] ? state.album[it.id].count : 1) + ' 尾'}</div></div><div class="rw-n" style="color:var(--lilac-deep)">${it.albumCount}/${D.FISH.length}</div></div>
        </div>
        <div class="sheet-actions"><button class="cta cta-primary" data-act="closeSheet">收入囊中</button><button class="cta cta-ghost" data-act="cast">再抛一竿</button></div>
      `);
    }
    const rows = list.map((it) => {
      const r = D.RARITY[it.rarity] || D.RARITY.common;
      return `<div class="batch-row">
        <div class="br-ic" style="background:linear-gradient(135deg,${r.color}cc,${r.color})">${icon('fish')}</div>
        <div class="br-meta"><div class="br-nm">${esc(it.name)} ${it.isNew ? '<span class="br-new">NEW</span>' : ''}</div><div class="br-pri">${rarTag(it.rarity)} · ${it.size}cm · ${it.dest === 'aquarium' ? '暂存鱼缸' : '入包'}</div></div>
        <div class="br-n">+${fmt(it.magic)}</div>
      </div>`;
    }).join('');
    const sum = list.reduce((a, it) => a + it.magic, 0);
    return sheetWrap(`
      <div class="catch-title" style="margin-bottom:4px">连竿 ${list.length} 尾</div>
      <div class="catch-sub">一竿多尾 · 逐尾结算 · 新鱼种已自动入鉴</div>
      <div class="batch-list">${rows}</div>
      <div class="batch-sum"><span class="bs-k">合计估值</span><span class="bs-v">✦ ${fmt(sum)}</span></div>
      <div class="sheet-actions"><button class="cta cta-primary" data-act="closeSheet">全部收下</button><button class="cta cta-ghost" data-act="cast">再抛一竿</button></div>
    `);
  }

  function fishSheet(state, opts) {
    const f = D.fishById[opts.id] || (state.bag.fish.find((x) => x.uid === opts.uid) ? D.fishById[state.bag.fish.find((x) => x.uid === opts.uid).id] : null);
    if (!f) return '';
    const r = D.RARITY[f.rarity];
    const rec = state.album[f.id];
    const got = !!rec;
    const condTxt = `${D.sceneById[f.scene].name} · ${condName(f.cond)}${f.needFloat ? ' · 需装备' + (D.itemById[f.needFloat] || {}).name : ''}`;
    const inst = opts.uid ? state.bag.fish.find((x) => x.uid === opts.uid) : null;

    return sheetWrap(`
      <div class="modal-hero">
        <div class="mh-box" style="background:linear-gradient(160deg,${r.color}bb,${r.color})">${icon('fish')}</div>
        <div>
          <div class="mh-nm">${got ? esc(f.name) : '未收录鱼种'} ${rarTag(f.rarity)}</div>
          <div class="mh-sub">${got ? condTxt : '？ · 出没条件待探索'}</div>
        </div>
      </div>
      <div class="attr-list">
        <div class="al-t">钓获条件</div>
        <div class="attr-row"><span class="ar-k">钓场</span><span class="ar-v">${esc(D.sceneById[f.scene].name)}</span></div>
        <div class="attr-row"><span class="ar-k">时段 / 天气</span><span class="ar-v">${condName(f.cond)}</span></div>
        <div class="attr-row"><span class="ar-k">体型范围</span><span class="ar-v">${f.size[0]}–${f.size[1]}cm</span></div>
        <div class="attr-row"><span class="ar-k">基准魔力</span><span class="ar-v">✦${f.value}</span></div>
        ${f.needFloat ? `<div class="attr-row"><span class="ar-k">特殊要求</span><span class="ar-v">${esc((D.itemById[f.needFloat] || {}).name || '')}</span></div>` : ''}
      </div>
      ${rec ? `<div class="attr-list"><div class="al-t">我的记录</div>
        <div class="attr-row"><span class="ar-k">累计钓获</span><span class="ar-v">${rec.count} 尾</span></div>
        <div class="attr-row"><span class="ar-k">最大体型</span><span class="ar-v">${rec.bestSize || '—'}cm</span></div>
        <div class="attr-row"><span class="ar-k">单尾最高魔力</span><span class="ar-v">✦${rec.best || '—'}</span></div></div>` : ''}
      ${inst ? `<div class="attr-list"><div class="al-t">本条渔获</div>
        <div class="attr-row"><span class="ar-k">体型 / 重量</span><span class="ar-v">${inst.size}cm · ${inst.weight}kg</span></div>
        <div class="attr-row"><span class="ar-k">渔获估值</span><span class="ar-v">✦${inst.magic}</span></div>
        <div class="attr-row"><span class="ar-k">钓获时间</span><span class="ar-v">${new Date(inst.at).toLocaleString('zh-CN', { hour12: false })}</span></div></div>
        <div class="sheet-actions"><button class="cta cta-ghost" data-act="releaseBagFish" data-uid="${inst.uid}">放生（腾出 1 格）</button></div>` : ''}
      ${!inst ? `<div class="sheet-actions"><button class="cta cta-primary" data-act="gotoScene" data-id="${f.scene}" data-cond="${f.cond}">前往该钓场</button></div>` : ''}
    `);
  }

  function sceneSheet(state) {
    const sc = D.sceneById[state.scene];
    const conds = [
      { id: 'day', name: '白天', desc: '常规鱼群 · 视野最佳', bonus: '无额外修正' },
      { id: 'night', name: '夜钓', desc: '夜行鱼出没 · 夜钓装备生效', bonus: '稀有鱼概率随夜钓加成提升' },
      { id: 'rain', name: '阴雨', desc: '雨水扰动水面 · 咬钩更急', bonus: '咬钩速度 +15%' },
    ];
    const rows = conds.map((c) => `<div class="item-card" data-act="cond" data-c="${c.id}" style="cursor:pointer">
        <div class="ic-box ${tone(c.id === 'night' ? 'deep' : c.id === 'rain' ? 'sky' : 'sun')}">${icon(condIcon(c.id))}</div>
        <div class="im"><div class="im-top"><span class="im-nm">${c.name}</span>${state.cond === c.id ? '<span class="rar r-fine">当前</span>' : ''}</div>
        <div class="im-attr">${c.desc}</div><div class="im-own">${c.bonus}</div></div>
        <span class="act ${state.cond === c.id ? 'equipped' : 'owned'}">${state.cond === c.id ? '使用中' : '切换'}</span>
      </div>`).join('');

    return sheetWrap(`
      <div class="sheet-t">${esc(sc.name)} · 时段与天气</div>
      <div class="sheet-s">${esc(sc.desc)} · 时段会切换鱼种池，稀有度与咬钩速度随之变化。</div>
      <div class="item-list" style="padding:0">${rows}</div>
    `);
  }

  function rulesSheet(state) {
    const C = D.CONST;
    const stats = E.statsOf(state);
    return sheetWrap(`
      <div class="sheet-t">玩法规则</div>
      <div class="sheet-s">甜梦钓场 · 休闲养成向钓鱼玩法，所有数值均由配置表驱动。</div>
      <div class="attr-list">
        <div class="al-t">主链路</div>
        <div class="attr-row"><span class="ar-k">1 · 抛竿</span><span class="ar-v">消耗 1 体力（免费次数优先）</span></div>
        <div class="attr-row"><span class="ar-k">2 · 等待咬钩</span><span class="ar-v">2.4–5.0s ÷ (1 + 咬钩速度)</span></div>
        <div class="attr-row"><span class="ar-k">3 · 起竿窗口</span><span class="ar-v">当前 ${Math.round(stats.window)}ms</span></div>
        <div class="attr-row"><span class="ar-k">4 · 溜鱼</span><span class="ar-v">按住收线，张力不越线</span></div>
        <div class="attr-row"><span class="ar-k">5 · 结算</span><span class="ar-v">图鉴收录 + 订单进度（渔获仅计估值）</span></div>
      </div>
      <div class="attr-list">
        <div class="al-t">核心数值</div>
        <div class="attr-row"><span class="ar-k">体力</span><span class="ar-v">上限 ${C.staminaMax} · ${C.staminaRecoverMs / 60000} 分钟 / 点 · 补满 ✦${C.staminaRefillCost}</span></div>
        <div class="attr-row"><span class="ar-k">背包</span><span class="ar-v">${C.bagTiers.map((t) => t.cap).join(' / ')} 格 · 扩容 ✦${C.bagTiers.slice(1).map((t) => t.price).join(' / ')}</span></div>
        <div class="attr-row"><span class="ar-k">自动垂钓</span><span class="ar-v">一次买断 ✦${C.autoUnlockPrice}</span></div>
        <div class="attr-row"><span class="ar-k">订单</span><span class="ar-v">每日 3 单 · 全清额外 ✦${C.orderAllBonus}</span></div>
        <div class="attr-row"><span class="ar-k">社交</span><span class="ar-v">每日赠饵 ${C.giftDailyLimit} 次 · 可收 ${C.giftReceiveCap} 份</span></div>
        <div class="attr-row"><span class="ar-k">赛季</span><span class="ar-v">${C.seasonDays} 天 · 前 3 名 ✦500 / 300 / 200</span></div>
      </div>
      <div class="compare">存档保存在浏览器本地（localStorage），换设备不继承。断线判定：张力触顶即断线，当次渔获丢失。</div>
      <div class="sheet-actions">
        <button class="cta cta-ghost" data-act="exportSave">导出存档</button>
        <button class="cta cta-ghost" data-act="resetConfirm" style="color:var(--danger)">重置进度</button>
      </div>
    `);
  }

    function confirmSheet(opts) {
      return sheetWrap(`
        <div class="sheet-t">${esc(opts.title)}</div>
        <div class="sheet-s">${esc(opts.sub || '')}</div>
        <div class="sheet-actions">
          <button class="cta cta-ghost" data-act="closeSheet">取消</button>
          <button class="cta ${opts.danger ? 'cta-gold' : 'cta-primary'}" data-act="${opts.okAct}">${esc(opts.okText || '确认')}</button>
        </div>
      `);
    }

  function setPartsSheet(state, setId) {
    const s = D.setById[setId];
    if (!s) return '';
    const rows = Object.keys(s.parts).map((cat, i) => {
      const it = D.itemById[s.parts[cat]];
      const has = E.isOwned(state, cat, s.parts[cat]);
      const on = state.equipped[cat] === s.parts[cat];
      return `<div class="item-card" data-act="openItem" data-cat="${cat}" data-id="${s.parts[cat]}">
        <div class="ic-box ${tone(catTone(cat))}">${icon(D.catById[cat].icon)}</div>
        <div class="im"><div class="im-top"><span class="im-nm">${esc(it.name)}</span>${rarTag(it.rarity)}</div>
          <div class="im-attr">${D.catById[cat].name} · ${attrLine(it.eff)}</div>
          <div class="im-own">${has ? (on ? '已装备' : '已拥有') : '未拥有 · ✦' + it.price}</div></div>
        <span class="act ${on ? 'equipped' : has ? 'owned' : 'buy'}">${on ? '使用中' : has ? '装备' : '✦' + fmt(it.price)}</span>
      </div>`;
    }).join('');
    return sheetWrap(`
      <div class="sheet-t">${esc(s.name)} · 套装清单</div>
      <div class="sheet-s">需在 ${esc(D.sceneById[s.scene].name)} 集齐 6 件（场景 + 5 件渔具）触发齐套加成。</div>
      <div class="item-list" style="padding:0">${rows}</div>
    `);
  }

  function friendSheet(state, friendId) {
    const f = E.friendView(state, Date.now()).find((x) => x.id === friendId);
    if (!f) return '';
    const pond = f.pond.map((p) => {
      const r = D.RARITY[p.rarity];
      return `<div class="aqua-row"><div class="ar-ic" style="background:linear-gradient(135deg,${r.color}cc,${r.color})">${icon('fish')}</div>
        <div class="ar-meta"><div class="ar-nm">${esc(p.name)} ${rarTag(p.rarity)}</div><div class="ar-pri">${esc(D.sceneById[p.scene].name)} · ${condName(p.cond)} · 体型 ${p.size[0]}–${p.size[1]}cm</div></div></div>`;
    }).join('');
    return sheetWrap(`
      <div class="modal-hero">
        <div class="mh-box ${tone(f.tone)}" style="border-radius:50%">${esc(f.tag)}</div>
        <div><div class="mh-nm">${esc(f.name)} <span class="fr-tag">${f.online ? '在线' : '离线'}</span></div>
        <div class="mh-sub">常驻 ${esc(D.sceneById[f.scene].name)} · 使用 ${esc(f.rod)}</div></div>
      </div>
      <div class="sheet-s">鱼塘陈列（今日展示的 3 尾渔获）</div>
      <div class="aqua-list" style="padding:0">${pond}</div>
      <div class="sheet-actions"><button class="cta cta-gold" data-act="gift" data-id="${f.id}">赠送鱼饵</button></div>
    `);
    }

  /* ---------------- 导出 ---------------- */
    const VIEWS = {
      pond: renderPond, shop: renderShop, gear: renderGear, bag: renderBag,
      album: renderAlbum, orders: renderOrders, stamina: renderStamina,
      friends: renderFriends, ach: renderAch, rank: renderRank, aquarium: renderAquarium,
    };

    function render(state, view, ctx) {
      const fn = VIEWS[view] || renderPond;
      return fn(state, ctx || {});
    }

    G.FTUI = {
      render, icon, fmt, esc, rarTag, tone, attrLine, condName, condIcon, mmss,
      itemSheet, growSheet, catchSheet, fishSheet, sceneSheet, rulesSheet,
      setPartsSheet, friendSheet, confirmSheet, sheetWrap,
      catTone, sceneTone,
    };
})(typeof window !== 'undefined' ? window : globalThis);
