import { beforeEach, describe, expect, it, vi } from "vitest";

const { post, retryable } = vi.hoisted(() => ({
  post: vi.fn(),
  retryable: { value: false },
}));
vi.mock("@/lib/api-client", () => ({
  api: { post },
  isRetryableNetError: () => retryable.value,
}));
vi.mock("@/lib/client-uuid", () => ({
  clientUuid: () => "test-uuid",
}));

import { drawWheelMany } from "@/lib/wheel-batch";

/**
 * 连抽首选服务端批量事务（/games/wheel/batch，整组同生共死）；
 * 批量端点网络不可达时回落逐发单抽：每发独立幂等键、失败即停、已结算不吞。
 */
describe("转盘连抽", () => {
  beforeEach(() => {
    post.mockReset();
    retryable.value = false;
  });

  const isBatch = (c: unknown[]) => String(c[0]).endsWith("/batch");

  it("优先走批量端点：count+幂等键一次提交，rows 原样交回", async () => {
    post.mockImplementation(async (_path: unknown, body?: unknown) => {
      const b = body as { count: number; idempotency_key: string };
      expect(b.count).toBe(4);
      return {
        rows: Array.from({ length: b.count }, (_, i) => ({
          index: 1,
          prize: "1.5x 魔力",
          mult: 1.5,
          payout: 150,
          net: 50,
          g: i,
        })),
        total_net: 200,
        count: b.count,
      };
    });
    const out = await drawWheelMany<{ g: number }>(4);
    expect(out.error).toBeUndefined();
    expect(out.results).toHaveLength(4);
    expect(post).toHaveBeenCalledTimes(1);
    expect(isBatch(post.mock.calls[0])).toBe(true);
    expect(new Set(out.results.map((r) => r.g)).size).toBe(4);
  });

  it("批量端点 4xx（限次/校验）原样透传，不发逐发请求", async () => {
    post.mockImplementation(async () => {
      throw new Error("rate limited");
    });
    const out = await drawWheelMany(10);
    expect(out.results).toHaveLength(0);
    expect(out.error).toBeInstanceOf(Error);
    expect(out.skipped).toBe(10);
    expect(post).toHaveBeenCalledTimes(1);
  });

  it("批量端点网络层失败回落逐发单抽：每发独立幂等键", async () => {
    retryable.value = true;
    post.mockImplementation(async (path: unknown, body?: unknown) => {
      if (isBatch([path])) throw new Error("timeout");
      return {
        index: 1,
        prize: "谢谢参与",
        payout: 0,
        net: -100,
        key: (body as { idempotency_key?: string })?.idempotency_key ?? "",
      };
    });
    const out = await drawWheelMany<{ key: string }>(3);
    expect(out.error).toBeUndefined();
    expect(out.results).toHaveLength(3);
    expect(new Set(out.results.map((r) => r.key)).size).toBe(3);
    expect(post.mock.calls.filter(isBatch)).toHaveLength(1);
  });

  it("回落路径：第 N 发失败就停，已结算的原样交回", async () => {
    retryable.value = true;
    let n = 0;
    post.mockImplementation(async (path: unknown, body?: unknown) => {
      if (isBatch([path])) throw new Error("timeout");
      n++;
      // 第 3 发连补发一起失败：首发网络错，补发仍网络错 → 停发后续
      if (n === 3 || n === 4) throw new Error("timeout");
      return {
        index: 1,
        prize: "谢谢参与",
        payout: 0,
        net: -100,
        key: (body as { idempotency_key?: string })?.idempotency_key ?? "",
      };
    });
    const out = await drawWheelMany<{ key: string }>(
      6,
      "/api/v1/games/wheel",
      1,
    );
    expect(out.results).toHaveLength(2);
    expect(out.error).toBeInstanceOf(Error);
    // attempted 含失败的发次：发1✓ 发2✓ 发3✗（补发仍✗）→ 没发出的 3 发
    expect(out.skipped).toBe(3);
  });

  it("回落路径：网络失败用同一幂等键补发，服务端回放原结果", async () => {
    retryable.value = true;
    let n = 0;
    post.mockImplementation(async (path: unknown, body?: unknown) => {
      if (isBatch([path])) throw new Error("timeout");
      n++;
      if (n === 1) throw new Error("timeout");
      return {
        index: 1,
        prize: "1.5x 魔力",
        payout: 150,
        net: 50,
        key: (body as { idempotency_key?: string })?.idempotency_key ?? "",
      };
    });
    const out = await drawWheelMany(1, "/api/v1/games/wheel", 1);
    expect(out.results).toHaveLength(1);
    expect(out.error).toBeUndefined();
    const singles = post.mock.calls.filter((c) => !isBatch(c));
    expect(singles).toHaveLength(2);
    const keys = singles.map(
      (c) => (c[1] as { idempotency_key: string }).idempotency_key,
    );
    expect(new Set(keys).size).toBe(1);
  });
});
