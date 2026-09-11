import { describe, it, expect, vi, beforeEach } from "vitest";
import { api, ApiError, setSessionCookie, TOKEN_COOKIE } from "@/lib/api-client";

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
  document.cookie = "";
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
      json: () => Promise.resolve({ code: 2001, message: "未认证", data: null }),
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

  it("localStorage 有 token 时注入 Bearer", async () => {
    localStorage.setItem("flux.token", "tok123");
    const f = mockFetch();
    await api.get("/api/v1/me");
    const init = f.mock.calls[0][1] as RequestInit;
    expect((init.headers as Record<string, string>).Authorization).toBe("Bearer tok123");
  });

  it("无 token 时不带 Authorization", async () => {
    const f = mockFetch();
    await api.get("/api/v1/site-profile");
    const init = f.mock.calls[0][1] as RequestInit;
    expect((init.headers as Record<string, string>).Authorization).toBeUndefined();
  });

  it("post 序列化 body 且 method 正确", async () => {
    const f = mockFetch();
    await api.post("/api/v1/appeals", { kind: "hr", body: "x" });
    const init = f.mock.calls[0][1] as RequestInit;
    expect(init.method).toBe("POST");
    expect(init.body).toBe(JSON.stringify({ kind: "hr", body: "x" }));
  });

  it("del 走 DELETE 方法", async () => {
    const f = mockFetch();
    await api.del("/api/v1/torrents/1");
    expect((f.mock.calls[0][1] as RequestInit).method).toBe("DELETE");
  });

  it("call spec: PUT 带对象体", async () => {
    const f = mockFetch();
    await api.call<{ n: number }>('PUT /api/v1/torrents/9 {"small_descr":"x"}');
    const init = f.mock.calls[0][1] as RequestInit;
    expect(init.method).toBe("PUT");
    expect(init.body).toBe('{"small_descr":"x"}');
  });

  it("call spec 非法格式同步抛 1002", () => {
    expect(() => api.call("NOTAMETHOD /x")).toThrowError(expect.objectContaining({ code: 1002 }));
  });
});

describe("setSessionCookie", () => {
  it("写 token 时落两个 cookie", () => {
    setSessionCookie("t");
    expect(document.cookie).toContain("flux.session=1");
    expect(document.cookie).toContain(`${TOKEN_COOKIE}=t`);
  });
  it("清 token 时 cookie 过期", () => {
    setSessionCookie("t");
    setSessionCookie(null);
    expect(document.cookie).not.toContain("flux.session=1");
  });
});
