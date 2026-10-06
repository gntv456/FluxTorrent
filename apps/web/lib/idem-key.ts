/**
 * 发放类操作的幂等键（0291）。
 *
 * 口径只有一份：**同一份提交内容，在同一天里算同一个键。**
 *
 * 为什么必须有它：后端已经强制要求 `idempotency_key`（8~120 字符）并靠唯一约束
 * 拒绝重放，但早先的两处前端各自用 `Date.now()` 现造键（批量发放、抽卡券/碎片
 * 发放）。每次点击都是新键 ⇒ 服务端那层防护永远不会命中，双击就是两笔真发放。
 * `admin-gacha.tsx` 的文件注释甚至写着「同参数重提会被 400 拒绝」——那是意图，
 * 不是当时的实现。
 *
 * 按天分桶是为了不把站长按月重复的正常操作永久锁死：月度奖补参数常常一模一样，
 * 跨天必须是两批；同一天里连点/改完又点回原样，才算同一批。
 */

/** FNV-1a 32 位——把内容压成定长片段，不是安全散列。 */
function seg(s: string): string {
  let h = 2166136261;
  for (let i = 0; i < s.length; i++) {
    h ^= s.charCodeAt(i);
    h = Math.imul(h, 16777619);
  }
  return (h >>> 0).toString(16).padStart(8, "0");
}

/**
 * @param payload 参与判重的提交内容（不含 dry_run 这类不发放的开关）
 * @param prefix  端点前缀，避免不同发放口之间的键相互撞车
 */
export function idemKey(payload: unknown, prefix = "bulk"): string {
  const day = new Date().toISOString().slice(0, 10);
  return `${prefix}-${seg(JSON.stringify(payload))}-${seg(day)}`;
}
