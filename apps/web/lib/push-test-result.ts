/** /push/test 的读数与回话（P-570）。
 *
 *  旧实现把 ServiceError 并进 delivered、界面又不读响应体，于是一次
 *  根本没送达的测试也显示「测试消息已发送」。判定与文案抽成纯函数，
 *  失败分支不必造出真的推送服务故障才能验证。
 */
export type PushTestTally = {
  delivered?: number;
  failed?: number;
  expired_cleaned?: number;
};

export type PushTestCopy = {
  testSent: string;
  testPartial: string;
  testFailed: string;
  testExpired: string;
};

export function pushTestMessage(r: PushTestTally, t: PushTestCopy): string {
  const ok = r.delivered ?? 0;
  const failed = r.failed ?? 0;
  const gone = r.expired_cleaned ?? 0;
  if (ok > 0) {
    return failed + gone > 0 ? t.testPartial : t.testSent;
  }
  return gone > 0 ? t.testExpired : t.testFailed;
}
