"use client";

/**
 * 幂等键用途的客户端 UUID（2026-10-04 生产站实测修复）。
 *
 * crypto.randomUUID 只在 secure context（https / localhost）可用：
 * 站点冷启动期用户经 http://IP:3000 访问时它是 undefined——此前商店/银行/
 * 邀请/勋章/促销/装扮与全部游戏页在组件初始化里直接调用它，水合当场抛
 * TypeError，整页落 error.tsx「页面出错了」。
 *
 * 这里仅用于幂等键/动画 key，非安全场景：安全上下文用真 randomUUID，
 * 否则回落「时间戳+计数器+随机数」。禁止把本函数用于令牌/盐等安全用途。
 */

let seq = 0;

export function clientUuid(): string {
  if (
    typeof crypto !== "undefined" &&
    typeof crypto.randomUUID === "function"
  ) {
    try {
      return crypto.randomUUID();
    } catch {
      // 某些旧内核 secure context 判定异常时也可能抛——继续走回落
    }
  }
  seq = (seq + 1) % 0xffff;
  return `${Date.now().toString(36)}-${seq.toString(36)}-${Math.random()
    .toString(36)
    .slice(2, 10)}`;
}
