import { afterEach, describe, expect, it, vi } from "vitest";
import {
  CSRF_COOKIE,
  CSRF_HEADER,
  api,
  ApiError,
} from "@/lib/api-client";

/**
 * 批量收种的客户端接线（批二百零五 P-431）。
 *
 * 旧形状是组件内裸 fetch：不带 x-csrf-token，也够不到 csrfHeaderAsync 的续期。
 * 活体 A/B 实测只带 flux_token 的 POST 被 csrf_mw 判 403/2003「无权限」，
 * 补同名头才是 200 ⇒ 批量收种在真浏览器里整条不可用，而界面只显示「加载失败」。
 * 现在走 api.postBlob，本文件钉三件事：CSRF 头真带上、X-Batch-Skipped 真回传、
 * 失败信封真被读成服务端那句中文。
 *
 * 负控：删掉 blobRequest 里 csrfHeaderAsync 那一行 ⇒ 第 1 条红；
 * 把返回的 skipped 写死 0 ⇒ 第 2 条红；把 !res.ok 分支换回
 * `new ApiError(1000, "下载失败（HTTP 403）")` ⇒ 第 3 条红。
 */

function stubResp(overrides: Partial<Response>) {
  const fn = vi.fn(async (_url: string, _init?: RequestInit) =>
    ({
      ok: true,
      status: 200,
      headers: new Headers({ "content-type": "application/zip" }),
      arrayBuffer: async () => new Uint8Array([100]).buffer,
      ...overrides,
    }) as unknown as Response);
  vi.stubGlobal("fetch", fn);
  return fn;
}

afterEach(() => {
  vi.unstubAllGlobals();
  for (const c of document.cookie.split("; ")) {
    const n = c.split("=")[0].trim();
    if (n) document.cookie = `${n}=; path=/; max-age=0`;
  }
});

describe("api.postBlob（批量收种）", () => {
  it("POST 必须带 x-csrf-token（与 flux_csrf cookie 同值）", async () => {
    document.cookie = CSRF_COOKIE + "=tok9; path=/";
    const f = stubResp({});
    await api.postBlob("/api/v1/torrents/batch-download", { ids: [1, 2] });
    const [, init] = f.mock.calls[0];
    expect(init?.method).toBe("POST");
    expect((init?.headers as Record<string, string>)[CSRF_HEADER]).toBe("tok9");
    expect(init?.body).toBe('{"ids":[1,2]}');
  });

  it("X-Batch-Skipped 必须回传成 skipped（界面按实际件数报名字）", async () => {
    stubResp({
      headers: new Headers({
        "content-type": "application/zip",
        "x-batch-skipped": "2",
      }),
    });
    const r = await api.postBlob("/api/v1/torrents/batch-download", {
      ids: [1, 2, 3],
    });
    expect(r.skipped).toBe(2);
    expect(r.data.byteLength).toBe(1);
  });

  it("缺 CSRF 时被拒的 403 要读成服务端那句中文，不是状态码", async () => {
    stubResp({
      ok: false,
      status: 403,
      headers: new Headers({ "content-type": "application/json" }),
      json: async () => ({
        code: 2003,
        message: "无权限",
        data: null,
        request_id: "r-431",
      }),
    });
    const e = (await api
      .postBlob("/api/v1/torrents/batch-download", { ids: [1] })
      .catch((x) => x)) as ApiError;
    expect(e).toBeInstanceOf(ApiError);
    expect(e.code).toBe(2003);
    expect(e.message).toBe("无权限");
    expect(e.status).toBe(403);
    expect(e.requestId).toBe("r-431");
  });
});
