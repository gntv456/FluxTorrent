import { api, isRetryableNetError } from "@/lib/api-client";
import { clientUuid } from "@/lib/client-uuid";

/** 单次转盘结算（`POST /api/v1/games/wheel` 的返回形状）。 */
export interface WheelDrawResult {
  index: number;
  prize: string;
  payout: number;
  net: number;
  kind?: string;
  /** 物品档发不出去时的原因；有值即回落结算 */
  fell_back?: string;
  value?: number;
  rarity?: number;
  replay?: boolean;
  ticket?: number;
  /** 魔力档的语言无关倍率读数（0.5/1.5/20…）；物品档为 0 */
  mult?: number;
}

/** 批量端点（`POST /api/v1/games/wheel/batch`）的返回形状。 */
interface WheelBatchResponse {
  rows: WheelDrawResult[];
  total_net: number;
  count: number;
}

export interface WheelBatchOutcome<T> {
  /** 已结算的结果（按发出顺序；并发下可能轻微乱序） */
  results: T[];
  /** 第一发失败的原因；全部成功时为 undefined */
  error?: unknown;
  /** 因失败提前停下、没发出的发数 */
  skipped: number;
}

/**
 * 连抽首选**服务端批量事务**（`/games/wheel/batch`）：N 局一个事务，
 * 限次/余额/扣票/结算整组同生共死——余额只够 3 局时百连就是整组 3 局，
 * 不会出现「3 发已受理 + 97 发 4xx」的半截账。
 *
 * 批量端点不可用（旧后端/网络层拒绝）时回落到逐发单抽：每发独立幂等键、
 * 并发上限 5；网络超时用同一键补发一次，服务端按回放表返回原结果。
 * 「一直抽」仍走单抽（要逐局出闪字演出），不经过这里。
 */
export async function drawWheelMany<T>(
  count: number,
  path = "/api/v1/games/wheel",
  concurrency = 5,
): Promise<WheelBatchOutcome<T>> {
  const key = clientUuid();
  try {
    const r = await api.post<WheelBatchResponse>(
      `${path}/batch`,
      { count, idempotency_key: key },
    );
    return { results: r.rows as T[], skipped: 0 };
  } catch (batchErr) {
    // 4xx（限次/校验）不是端点缺失，透传给调用方按业务错处理
    if (!isRetryableNetError(batchErr)) {
      return { results: [], error: batchErr, skipped: count };
    }
  }
  return drawWheelSequential<T>(count, path, concurrency);
}

/** 回落路径：逐发真实单抽（旧实现原样保留）。 */
async function drawWheelSequential<T>(
  count: number,
  path: string,
  concurrency: number,
): Promise<WheelBatchOutcome<T>> {
  const base = clientUuid();
  const slots: (T | undefined)[] = new Array(count);
  let next = 0;
  let attempted = 0;
  let failure: unknown;
  let stopped = false;

  async function worker() {
    while (!stopped) {
      const i = next++;
      if (i >= count) return;
      attempted++;
      const key = `${base}-b${i}`;
      try {
        slots[i] = await api.post<T>(path, { idempotency_key: key });
      } catch (e) {
        // 网络超时不等于服务端没结算；用同一幂等键补一次，服务端会回放原结果。
        // 4xx（限次/校验）不重试，避免把无效请求打穿。
        if (isRetryableNetError(e)) {
          try {
            slots[i] = await api.post<T>(path, { idempotency_key: key });
            continue;
          } catch (retryErr) {
            stopped = true;
            failure = retryErr;
            return;
          }
        }
        stopped = true;
        failure = e;
      }
    }
  }

  await Promise.all(
    Array.from({ length: Math.min(concurrency, count) }, () => worker()),
  );

  const results = slots.filter((r): r is T => r !== undefined);
  return { results, error: failure, skipped: count - attempted };
}
