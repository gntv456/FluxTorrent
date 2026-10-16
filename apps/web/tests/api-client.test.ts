import { describe, it, expect, vi, beforeEach, afterEach } from "vitest";
import {
  api,
  ApiError,
  setSessionCookie,
  hasSessionCookie,
  csrfToken,
  CSRF_COOKIE,
  CSRF_HEADER,
} from "@/lib/api-client";

/** api-client 单元测试：envelope 解包 / 错误归一 / token 注入 / call spec */
function mockFetch(overrides: Partial<Response> = {}) {
  const fn = vi.fn().mockResolvedValue({
    ok: true,
    status: 200,
    headers: new Headers({ "content-type": "application/json" }),
    json: () => Promise.resolve({ code: 0, message: "ok", data: { n: 1 } }),
    ...overrides,
  } as unknown as Response);
  vi.stubGlobal("fetch", fn);
  return fn;
}

beforeEach(() => {
  localStorage.clear();
  // jsdom：赋空串不会清 cookie，必须逐项过期
  for (const c of document.cookie.split("; ")) {
    const n = c.split("=")[0].trim();
    if (n) document.cookie = `${n}=; path=/; max-age=0`;
  }
});

describe("api client envelope", () => {
  it("code=0 解包返回 data", async () => {
    const f = mockFetch();
    const d = await api.get<{ n: number }>("/api/v1/anything");
    expect(d).toEqual({ n: 1 });
    expect(f).toHaveBeenCalledWith(
      "/api/v1/anything",
      expect.objectContaining({ cache: "no-store" }),
    );
  });

  it("code!=0 抛 ApiError 携带业务码", async () => {
    mockFetch({
      json: () =>
        Promise.resolve({ code: 2001, message: "未认证", data: null }),
    });
    await expect(api.get("/api/v1/me")).rejects.toMatchObject({
      code: 2001,
      message: "未认证",
    });
    await expect(api.get("/api/v1/me")).rejects.toBeInstanceOf(ApiError);
  });

  it("非 JSON 响应归一化 1000", async () => {
    mockFetch({
      ok: false,
      status: 502,
      headers: new Headers({ "content-type": "text/html" }),
    });
    await expect(api.get("/api/v1/me")).rejects.toMatchObject({ code: 1000 });
  });

  it("浏览器侧不再注入 Bearer（凭证由 HttpOnly cookie 自动携带）", async () => {
    // P1 收敛：token 不进 localStorage/JS 可读 cookie——浏览器请求依赖
    // 同源 cookie（fetch 默认 credentials: same-origin），Authorization 不应出现
    const f = mockFetch();
    await api.get("/api/v1/me");
    const init = f.mock.calls[0][1] as RequestInit;
    expect(
      (init.headers as Record<string, string>).Authorization,
    ).toBeUndefined();
  });

  it("无 token 时不带 Authorization", async () => {
    const f = mockFetch();
    await api.get("/api/v1/site-profile");
    const init = f.mock.calls[0][1] as RequestInit;
    expect(
      (init.headers as Record<string, string>).Authorization,
    ).toBeUndefined();
  });

  /** 按 URL 取那次调用的 init：写请求之前现在可能先打一次 /api/v1/me/csrf
   *  续期，位置断言（calls[0]）会把续期当成业务请求（2026-10-16 三条假红）。 */
  function initFor(f: ReturnType<typeof mockFetch>, url: string) {
    const hit = f.mock.calls.find((c) => String(c[0]).endsWith(url));
    return (hit?.[1] ?? {}) as RequestInit;
  }

  it("post 序列化 body 且 method 正确", async () => {
    const f = mockFetch();
    await api.post("/api/v1/appeals", { kind: "hr", body: "x" });
    const init = initFor(f, "/api/v1/appeals");
    expect(init.method).toBe("POST");
    expect(init.body).toBe(JSON.stringify({ kind: "hr", body: "x" }));
  });

  it("del 走 DELETE 方法", async () => {
    const f = mockFetch();
    await api.del("/api/v1/torrents/1");
    expect(initFor(f, "/api/v1/torrents/1").method).toBe("DELETE");
  });

  it("call spec: PUT 带对象体", async () => {
    const f = mockFetch();
    await api.call<{ n: number }>('PUT /api/v1/torrents/9 {"small_descr":"x"}');
    const init = initFor(f, "/api/v1/torrents/9");
    expect(init.method).toBe("PUT");
    expect(init.body).toBe('{"small_descr":"x"}');
  });

  it("缺 flux_csrf cookie 时，写请求前先续期一次（否则全站写都是 403）", async () => {
    const f = mockFetch();
    document.cookie = CSRF_COOKIE + "=; path=/; expires=Thu, 01 Jan 1970 00:00:00 GMT";
    await api.post("/api/v1/appeals", { kind: "hr", body: "x" });
    // 续期口只该在缺 cookie 时打一次，且它自己不带 CSRF 头（否则鸡生蛋）
    const renew = f.mock.calls.filter((c) =>
      String(c[0]).endsWith("/api/v1/me/csrf"),
    );
    expect(renew.length).toBe(1);
    const hdrs = (renew[0][1] as RequestInit).headers as Record<string, string>;
    expect(hdrs[CSRF_HEADER]).toBeUndefined();
    // 业务写请求仍照常发出（续期失败也不静默改写调用方语义）
    expect(initFor(f, "/api/v1/appeals").method).toBe("POST");
  });

  it("已有 flux_csrf cookie 时不再续期", async () => {
    document.cookie = CSRF_COOKIE + "=abc123; path=/";
    const f = mockFetch();
    await api.post("/api/v1/appeals", { kind: "hr" });
    expect(
      f.mock.calls.some((c) => String(c[0]).endsWith("/api/v1/me/csrf")),
    ).toBe(false);
    const init = initFor(f, "/api/v1/appeals");
    expect((init.headers as Record<string, string>)[CSRF_HEADER]).toBe("abc123");
  });

  it("call spec 非法格式同步抛 1002", () => {
    expect(() => api.call("NOTAMETHOD /x")).toThrowError(
      expect.objectContaining({ code: 1002 }),
    );
  });
});

describe("setSessionCookie / hasSessionCookie", () => {
  it("登录态写 flux.session 标记（token 已 HttpOnly 化，不经 JS）", () => {
    setSessionCookie(true);
    expect(document.cookie).toContain("flux.session=1");
    expect(hasSessionCookie()).toBe(true);
  });
  it("登出清标记", () => {
    setSessionCookie(true);
    setSessionCookie(false);
    expect(document.cookie).not.toContain("flux.session=1");
    expect(hasSessionCookie()).toBe(false);
  });
});

describe("CSRF 双提交（csrf.ts 契约）", () => {
  afterEach(() => {
    // jsdom 的 document.cookie 是会话 cookie：不逐个过期就会漏进下一条用例
    for (const c of document.cookie.split("; ")) {
      const n = c.split("=")[0].trim();
      if (n) document.cookie = `${n}=; path=/; max-age=0`;
    }
  });

  it("cookie 缺失时 csrfToken 返回 null", () => {
    expect(csrfToken()).toBeNull();
  });

  it("读得到 flux_csrf cookie 的值", () => {
    document.cookie = CSRF_COOKIE + "=abc123; path=/";
    expect(csrfToken()).toBe("abc123");
  });

  it("不是首位的 cookie 也能读到（分号后有空格的口径）", () => {
    document.cookie = "other=1; path=/";
    document.cookie = CSRF_COOKIE + "=xyz; path=/";
    expect(csrfToken()).toBe("xyz");
  });

  it("GET 不带 CSRF 头（安全方法少一处无谓头）", async () => {
    const f = mockFetch();
    document.cookie = CSRF_COOKIE + "=tok; path=/";
    await api.get("/api/v1/x");
    const h = f.mock.calls[0][1]?.headers as Record<string, string>;
    expect(h[CSRF_HEADER]).toBeUndefined();
  });

  it("POST 回填 X-CSRF-Token（与 cookie 同值）", async () => {
    document.cookie = CSRF_COOKIE + "=tok99; path=/";
    const f = mockFetch();
    await api.post("/api/v1/y", { a: 1 });
    const h = f.mock.calls[0][1]?.headers as Record<string, string>;
    expect(h[CSRF_HEADER]).toBe("tok99");
  });

  it("PUT/DELETE/PATCH 同样回填", async () => {
    for (const m of [
      () => api.put("/api/v1/y", {}),
      () => api.del("/api/v1/y"),
      () => api.patch("/api/v1/y", {}),
    ]) {
      const f = mockFetch();
      document.cookie = CSRF_COOKIE + "=t; path=/";
      await m();
      const h = f.mock.calls[0][1]?.headers as Record<string, string>;
      expect(h[CSRF_HEADER]).toBe("t");
    }
  });

  it("无 cookie 时写方法也不带该头（后端会 403，前端应能读到错误）", async () => {
    const f = mockFetch();
    await api.post("/api/v1/y", {});
    const h = f.mock.calls[0][1]?.headers as Record<string, string>;
    expect(h[CSRF_HEADER]).toBeUndefined();
  });
});
