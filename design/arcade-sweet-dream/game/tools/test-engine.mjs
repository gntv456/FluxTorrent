/* 逻辑层自测：node tools/test-engine.mjs
 * 用最简单的断言跑一遍主链路，确保配置表/引擎口径自洽，不被 UI 掩盖问题。
 */
import fs from 'node:fs';
import path from 'node:path';

const root = path.resolve(import.meta.dirname, '..');
const load = (rel) => {
  const code = fs.readFileSync(path.join(root, rel), 'utf8');
  new Function(code)();
};
load('js/data.js');
load('js/engine.js');

const D = globalThis.FTData;
const E = globalThis.FTEngine;

let pass = 0;
let fail = 0;
const ok = (cond, name, extra = '') => {
  if (cond) { pass++; console.log('  ✓ ' + name); }
  else { fail++; console.log('  ✗ ' + name + (extra ? ' → ' + extra : '')); }
};

console.log('\n[1] 数据完整性');
ok(D.SCENES.length === 4, '场景 4 个');
ok(D.FISH.length > 30, `鱼种 ${D.FISH.length} 条`);
ok(D.SETS.length === 4, '套装 4 套');
const dupFish = D.FISH.length - new Set(D.FISH.map((f) => f.id)).size;
ok(dupFish === 0, '鱼种 id 无重复', `重复 ${dupFish}`);
const dupItem = Object.keys(D.itemById).length;
const allItem = Object.values(D.ITEMS).flat().length;
ok(dupItem === allItem, '装备 id 唯一', `${dupItem} vs ${allItem}`);
// 套装里引用的装备必须存在
let setRefOk = true;
D.SETS.forEach((s) => {
  Object.entries(s.parts).forEach(([cat, id]) => {
    if (!D.itemById[id] || D.itemById[id].cat !== cat) setRefOk = false;
  });
});
ok(setRefOk, '套装引用装备存在且分类正确');

console.log('\n[2] 初始存档');
const st = E.defaultState(Date.now());
E.hydrate(st, Date.now());
ok(st.equipped.rod === 'bamboo', '默认鱼竿');
ok(E.bagCap(st) === 30, '默认背包 30 格');
ok(st.magic === 3200, '初始魔力 3200');

console.log('\n[3] 体力恢复');
st.stamina = { v: 4, at: 1000 };
E.recoverStamina(st, 1000 + 6 * 60000 * 3);
ok(st.stamina.v === 7, '6 分钟 / 点，恢复 3 点', `v=${st.stamina.v}`);
E.recoverStamina(st, 1000 + 6 * 60000 * 999);
ok(st.stamina.v === D.CONST.staminaMax, '上限封顶', `v=${st.stamina.v}`);

console.log('\n[4] 装备属性汇总');
const base = E.statsOf(st);
ok(base.window >= 800, '基础窗口 ≥ 800ms', String(base.window));
st.owned.rod.push({ id: 'moon', lv: 1 });
E.equipItem(st, 'rod', 'moon');
const withRod = E.statsOf(st);
ok(withRod.window === base.window + 400, '月光竿 +400ms 窗口', `${base.window}→${withRod.window}`);
ok(withRod.rare > 0, '稀有鱼概率加成生效');

console.log('\n[5] 套装激活');
st.scene = 'bay';
const cur = E.activeSet(st);
ok(cur === null, '混搭时无套装');
// 换成海湾新手套全套
['rod:bamboo', 'bait:bread', 'hook:iron', 'line:cotton', 'float:reed'].forEach((p) => {
  const [cat, id] = p.split(':');
  if (!E.isOwned(st, cat, id)) st.owned[cat].push({ id, lv: 1 });
  E.equipItem(st, cat, id);
});
const setNow = E.activeSet(st);
ok(setNow && setNow.id === 'bayNewbie', '六件齐备激活海湾新手套', setNow && setNow.id);
ok(E.statsOf(st).biteSpeed >= 0.05, '套装咬钩加成进入属性汇总');

console.log('\n[6] 抽鱼与抛竿');
let seed = 12345;
const rng = E.mulberry32(seed);
st.cond = 'day';
const f1 = E.rollFish(st, rng);
ok(!!f1 && f1.scene === 'bay', '抽到本场景鱼', f1 && f1.name);
ok(f1.cond === 'day', '抽到白天鱼种', f1.cond);
st.cond = 'night';
const f2 = E.rollFish(st, rng);
ok(f2.cond === 'night', '夜钓池生效', `${f2.name}/${f2.cond}`);

st.cond = 'day';
st.stamina = { v: 12, at: Date.now() };
st.freeCasts = 1;
const freeCast = E.cast(st, rng, Date.now());
ok(freeCast.ok && st.freeCasts === 0 && st.stamina.v === 12, '免费次数优先于体力');
st.stamina = { v: 12, at: Date.now() };
const castInfo = E.cast(st, rng, Date.now());
ok(castInfo.ok === true, '抛竿成功');
ok(castInfo.biteAfter >= 900 && castInfo.biteAfter <= 6000, '咬钩延迟在合理区间', String(castInfo.biteAfter));
ok(st.stamina.v === 11, '扣 1 点体力', String(st.stamina.v));

console.log('\n[7] 体力不足拦截');
st.stamina = { v: 0, at: Date.now() };
st.freeCasts = 0;
const c2 = E.cast(st, rng, Date.now());
ok(c2.ok === false && c2.reason === 'noStamina', '无体力不可抛竿');

console.log('\n[8] 溜鱼数值模拟');
st.stamina = { v: 12, at: Date.now() };
const ci = E.cast(st, rng, Date.now());
const rt = E.makeReelRuntime(ci, ci.fish);
let landed = false, broken = false, ticks = 0;
// 简单策略：张力低于 70% 就收，高于 70% 就松
for (let i = 0; i < 2400 && !landed && !broken; i++) {
  rt.holding = rt.tension < rt.softLimit * 0.8;
  const r = E.reelStep(rt, 0.016);
  ticks++;
  if (r.broken) broken = true;
  if (r.landed) landed = true;
}
ok(landed || broken, '溜鱼在限定时间内有结论', `landed=${landed} broken=${broken} ticks=${ticks}`);
ok(rt.tension <= rt.limit + 0.001, '张力不超过上限', String(rt.tension));

console.log('\n[9] 结算与入包');
const settle = E.settleCatch(st, ci.fish, ci, rng);
ok(settle.magic > 0, '产出魔力 > 0', String(settle.magic));
ok(settle.dest === 'bag', '默认入包', settle.dest);
ok(E.bagUsed(st) === E.gearOwnedCount(st) + st.bag.fish.length, '背包占用 = 渔具 + 渔获');
ok(st.album[ci.fish.id] && st.album[ci.fish.id].count === 1, '图鉴首次登记');
const settle2 = E.settleCatch(st, ci.fish, ci, rng);
ok(st.album[ci.fish.id].count === 2, '图鉴重复计数');

console.log('\n[10] 满包 → 临时鱼缸');
st.bag.fish = [];
st.bag.tier = 0;
const gear = E.gearOwnedCount(st);
for (let i = 0; i < 40; i++) {
  const r2 = E.settleCatch(st, ci.fish, ci, rng);
  if (i === 0) ok(r2.dest === 'bag', '首条入包');
}
ok(st.aquarium.length > 0, '溢出进临时鱼缸', String(st.aquarium.length));
ok(st.aquarium.length <= E.aquariumCap(st), '鱼缸不超容', `${st.aquarium.length}/${E.aquariumCap(st)}`);

console.log('\n[11] 购买 / 养成 / 扩容');
st.magic = 50000;
const buy = E.buyItem(st, 'rod', 'dragon');
ok(buy.ok === true, '购买沧海龙鳞竿');
ok(st.magic === 50000 - 2800, '扣款正确', String(st.magic));
const dup = E.buyItem(st, 'rod', 'dragon');
ok(dup.ok === false && dup.reason === 'owned', '已拥有不可重复购买');
const up = E.upgradeItem(st, 'rod', 'dragon');
ok(up.ok === true && up.lv === 2, '升级到 Lv.2');
const prev = E.upgradePreview(st, 'rod', 'dragon');
ok(prev.lv === 2 && prev.next !== null, '养成预览拿到下一阶');
let upCount = 1;
while (E.upgradeItem(st, 'rod', 'dragon').ok) upCount++;
ok(E.itemLv(st, 'rod', 'dragon') === D.CONST.upgradeMaxLv, '升级到满级上限', String(E.itemLv(st, 'rod', 'dragon')));
const capBefore = E.bagCap(st);
ok(E.expandBag(st).ok, '扩容成功');
ok(E.bagCap(st) > capBefore, '容量提升', `${capBefore}→${E.bagCap(st)}`);

console.log('\n[12] 场景解锁链路');
st.magic = 50000;
const s3 = E.buyScene(st, 'abyss');
ok(s3.ok === false && s3.reason === 'locked', '跨级解锁被拦截（需先解冰雪峡谷）');
const s2 = E.buyScene(st, 'gorge');
ok(s2.ok === true, '解锁冰雪峡谷');
const s3b = E.buyScene(st, 'abyss');
ok(s3b.ok === true, '解锁月夜深潭');

console.log('\n[13] 订单');
st.scenesOwned = ['bay', 'gorge', 'abyss', 'reef'];
E.ensureOrders(st, Date.now());
ok(st.orders.list.length === 3, '每日 3 单');
ok(st.orders.list.every((o) => o.need >= 1 && o.need <= 3), '需求量 1–3');
const o0 = st.orders.list[0];
o0.got = o0.need;
const cr = E.claimOrder(st, 0);
ok(cr.ok === true, '订单可领取');
ok(E.claimOrder(st, 0).ok === false, '不可重复领取');
ok(st.magic > 0, '奖励入账');

console.log('\n[14] 签到');
st.sign = { idx: 0, last: '', cycle: 0 };
st.magic = 0;
const sg = E.claimSign(st, Date.now());
ok(sg.ok === true && st.magic === 20, 'D1 领到 ✦20', String(st.magic));
ok(E.claimSign(st, Date.now()).ok === false, '同一天不可重复签到');
const sg2 = E.claimSign(st, Date.now() + 86400000);
ok(sg2.ok === true && sg2.idx === 1, '次日进入 D2', String(sg2.idx));

console.log('\n[15] 好友 / 赠饵');
const fs1 = E.friendState(st, Date.now());
fs1.sent = 0;
let giftOk = 0;
for (let i = 0; i < 7; i++) if (E.giftBait(st, 'f1', Date.now()).ok) giftOk++;
ok(giftOk === D.CONST.giftDailyLimit, `每日赠饵上限 ${D.CONST.giftDailyLimit}`, String(giftOk));
const fv = E.friendView(st, Date.now());
ok(fv.length === D.FRIENDS.length && fv[0].pond.length === 3, '好友视图生成');

console.log('\n[16] 排行 / 成就');
const board = E.rankBoard(st, Date.now());
ok(board.rows.length === D.RIVALS.length + 1, '排行含自己');
ok(board.rows[0].rank === 1 && board.rows[board.rows.length - 1].rank === board.rows.length, '名次连续');
const av = E.achView(st);
ok(av.length === D.ACH.length, '成就全部可评估');
ok(av.some((a) => a.done), '已有达成项');
const claimable = av.find((a) => a.claimable);
if (claimable) ok(E.claimAch(st, claimable.def.id).ok, '可领取成就入账');

console.log('\n[17] 图鉴分区');
const groups = E.albumView(st);
ok(groups.length >= 8, `分区数 ${groups.length}`);
ok(groups.every((g) => g.list.length > 0), '分区无空组');

console.log('\n[18] 存档容错');
const brokenSave = { magic: 0 };
const fixed = E.hydrate(brokenSave, Date.now());
ok(fixed.equipped.rod === 'bamboo', '残缺存档补默认装备');
ok(Array.isArray(fixed.bag.fish), '残缺存档补背包');
ok(typeof fixed.stats.casts === 'number', '残缺存档补统计');

console.log('\n[19] 性能：万次抽鱼');
const t0 = Date.now();
for (let i = 0; i < 10000; i++) E.rollFish(st, rng);
const cost = Date.now() - t0;
ok(cost < 400, `1 万次抽鱼 ${cost}ms（<400ms）`, String(cost));
const t1 = Date.now();
for (let i = 0; i < 10000; i++) E.statsOf(st);
ok(Date.now() - t1 < 400, `1 万次属性汇总 ${Date.now() - t1}ms`);

console.log('\n[20] 全链路连钓 60 次');
const g = E.defaultState(Date.now());
E.hydrate(g, Date.now());
g.magic = 50000;
g.scenesOwned = ['bay', 'gorge', 'abyss', 'reef'];
E.ensureOrders(g, Date.now());
const rng2 = E.mulberry32(999);
let landedN = 0; let brokenN = 0; let aquaN = 0; let lostN = 0; let castFails = 0;
for (let i = 0; i < 60; i++) {
  if (g.stamina.v <= 0) E.recoverStamina(g, Date.now() + 6 * 60000 * 100);
  const c = E.cast(g, rng2, Date.now() + i * 1000);
  if (!c.ok) { castFails++; continue; }
  const rt2 = E.makeReelRuntime(c, c.fish);
  let result = 'timeout';
  for (let t = 0; t < 3000; t++) {
    rt2.holding = rt2.tension < rt2.softLimit * 0.9;
    const r = E.reelStep(rt2, 0.016);
    if (r.broken) { result = 'broken'; break; }
    if (r.landed) { result = 'landed'; break; }
  }
  if (result === 'landed') {
    landedN++;
    const s = E.settleCatch(g, c.fish, c, rng2);
    if (s.dest === 'aquarium') aquaN++;
    else if (s.dest === 'lost') lostN++;
  } else brokenN++;
}
ok(castFails <= 1, '体力兜底后抛竿基本不失败', String(castFails));
ok(landedN + brokenN === 60 - castFails, '每次抛竿都有结局（不超时）', `landed=${landedN} broken=${brokenN}`);
// 全程硬拉，验证张力触顶会断线
const bk = E.defaultState(Date.now()); E.hydrate(bk, Date.now());
const bkRng = E.mulberry32(7);
let bkBroken = false;
for (let i = 0; i < 12 && !bkBroken; i++) {
  if (bk.stamina.v <= 0) E.recoverStamina(bk, Date.now() + 6 * 60000 * 100);
  const c = E.cast(bk, bkRng, Date.now());
  if (!c.ok) continue;
  const r = E.makeReelRuntime(c, c.fish);
  for (let t = 0; t < 3000; t++) {
    r.holding = true;
    const step = E.reelStep(r, 0.016);
    if (step.broken) { bkBroken = true; break; }
    if (step.landed) break;
  }
}
ok(bkBroken, '全程硬拉会断线');
ok(E.bagUsed(g) <= E.bagCap(g), '背包不超容', `${E.bagUsed(g)}/${E.bagCap(g)}`);
ok(g.aquarium.length <= E.aquariumCap(g), '鱼缸不超容', `${g.aquarium.length}/${E.aquariumCap(g)}`);
ok(Number.isFinite(g.magic) && g.magic > 0, '魔力不出现 NaN', String(g.magic));
ok(g.stats.casts === 60 - castFails, '抛竿计数与循环一致', `${g.stats.casts}`);
ok(g.album && Object.keys(g.album).length > 0, '图鉴有收录', String(Object.keys(g.album).length));
ok(g.orders.list.every((o) => o.got <= o.need), '订单进度不超过需求量');
ok(g.stats.catches === landedN, '钓获计数与落地数一致', `${g.stats.catches}/${landedN}`);

console.log('\n[21] 鱼缸取回 / 放生');
const aqState = E.defaultState(Date.now());
E.hydrate(aqState, Date.now());
aqState.bag.fish = [];
aqState.bag.tier = 3;
aqState.aquarium = [];
for (let i = 0; i < 12; i++) aqState.aquarium.push({ uid: 'aq' + i, fishId: D.FISH[0].id, name: D.FISH[0].name, rarity: D.FISH[0].rarity, w: 100, magic: 10, at: Date.now(), scene: 'bay' });
aqState.bag.fish = Array.from({ length: E.bagCap(aqState) - E.gearOwnedCount(aqState) }, (_, i) => ({ uid: 'b' + i, fishId: D.FISH[0].id, name: D.FISH[0].name, rarity: D.FISH[0].rarity, w: 100, magic: 10, at: Date.now(), scene: 'bay' }));
ok(E.takeFromAquarium(aqState, 'aq0').ok === false && E.takeFromAquarium(aqState, 'aq0').reason !== undefined, '背包满时不可取回');
E.releaseFromBag(aqState, aqState.bag.fish[0].uid);
ok(E.takeFromAquarium(aqState, 'aq0').ok === true, '腾出空格后可取回');
ok(E.releaseFromAquarium(aqState, 'aq0').reason === 'missing', '取走后原鱼缸条目消失');
E.releaseFromBag(aqState, aqState.bag.fish[0].uid);
const moved = E.takeAllFromAquarium(aqState);
ok(moved.moved > 0, '一键取回有搬运量', String(moved.moved));
ok(E.bagUsed(aqState) <= E.bagCap(aqState), '一键取回不撑爆背包');

console.log('\n[22] 赛季与刷新');
const sInfo = E.seasonInfo(g, Date.now());
ok(sInfo.ordinal >= 1 && sInfo.leftDays > 0 && sInfo.leftDays <= D.CONST.seasonDays, '赛季信息可用', `第 ${sInfo.ordinal} 赛季 · 剩 ${sInfo.leftDays} 天`);
ok(E.rankBoard(g, Date.now()).rows.every((r) => Number.isFinite(r.catches)), '排行分数为数值');
const nextDay = Date.now() + 86400000 * 2;
E.ensureOrders(g, nextDay);
ok(g.orders.list.length === 3, '跨日重生成订单');
ok(g.orders.bonusClaimed === false || g.orders.date !== E.todayKey(nextDay), '新一日订单奖励可再领');

console.log(`\n结果：${pass} 通过 / ${fail} 失败`);
process.exit(fail ? 1 : 0);
