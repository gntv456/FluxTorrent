import { describe, it, expect, vi, beforeEach, afterEach } from "vitest";
import { readFileSync, readdirSync, statSync } from "node:fs";
import { join, relative } from "node:path";

import { attachUpload } from "@/components/upload-attach-io";
import { uploadAttachment } from "@/components/subtitle-attachment";
import { zhCN } from "@/i18n/zh-CN";

/**
 * P-500：组件内裸 fetch 的写操作必须带 CSRF 双提交头。
 *
 * 后端 csrf_mw 对「cookie 鉴权 + 非安全方法 + /api/v1」要求
 * flux_csrf 与 x-csrf-token 逐字相等。现网实测：同样一份 multipart，
 * 不带头回 403/2003「无权限」，带头才越过闸门（400 业务校验）。
 * 附件上传、视频上传、预览维护、补签撤销此前全都只在 catch 里演成
 * 一句通用失败，所以这里既钉实际请求头，也钉全仓写口没有漏网的。
 */

let calls: { url: string; init?: RequestInit }[] = [];

function res(payload: unknown, status = 200): Response {
  return {
    ok: status >= 200 && status < 300,
    status,
    headers: { get: () => "application/json" },
    json: async () => payload,
  } as unknown as Response;
}

beforeEach(() => {
  calls = [];
  document.cookie = "flux_csrf=tok123; path=/";
  vi.stubGlobal(
    "fetch",
    vi.fn(async (input: RequestInfo | URL, init?: RequestInit) => {
      calls.push({ url: String(input), init });
      if (String(input).includes("/me/csrf")) {
        return res({ code: 0, data: null, request_id: "t" });
      }
      return res({
        code: 0,
        data: { url: "/u/a.png", sha256: "abc123" },
        request_id: "t",
      });
    }),
  );
});

afterEach(() => {
  vi.unstubAllGlobals();
  document.cookie = "flux_csrf=; path=/; max-age=0";
});

function headerOf(i = 0): Record<string, string> {
  return (calls[i]?.init?.headers ?? {}) as Record<string, string>;
}

describe("裸 fetch 写口的 CSRF 头", () => {
  it("发种附件的 attachUpload 带 x-csrf-token", async () => {
    let descr = "";
    await attachUpload(
      new File(["x"], "a.png", { type: "image/png" }),
      zhCN,
      (v) => {
        descr = typeof v === "function" ? v("") : v;
      },
    );
    expect(calls.some((c) => c.url.endsWith("/attachments"))).toBe(true);
    expect(headerOf()["x-csrf-token"]).toBe("tok123");
    expect(descr).toContain("/u/a.png");
  });

  it("字幕附件 uploadAttachment 带 x-csrf-token", async () => {
    await expect(
      uploadAttachment(new File(["x"], "a.srt")),
    ).resolves.toBe("abc123");
    expect(headerOf()["x-csrf-token"]).toBe("tok123");
  });
});

const ROOT = process.cwd();
const SCAN_DIRS = ["components", "app", "lib"];
const MUTATION = /method:\s*"(POST|PUT|PATCH|DELETE)"/g;

function walk(dir: string): string[] {
  return readdirSync(dir).flatMap((name) => {
    const p = join(dir, name);
    if (statSync(p).isDirectory()) {
      if (p.includes("node_modules") || p.includes(".next")) return [];
      return walk(p);
    }
    return /\.(ts|tsx)$/.test(name) ? [p] : [];
  });
}

/** CSRF 传输层自身：request() 内部注入头，它的 method 字面量不属写口。 */
const TRANSPORT = join("lib", "api-client.ts");

describe("全仓写口棘轮", () => {
  it("每个写方法 fetch 附近都要有 csrf 助手（含正控）", () => {
    const transport = readFileSync(join(ROOT, TRANSPORT), "utf8");
    expect(transport).toContain("csrfHeaderAsync");
    const files = SCAN_DIRS.flatMap((d) => walk(join(ROOT, d))).filter(
      (f) => !f.endsWith(TRANSPORT),
    );
    const sites = files.flatMap((f) => {
      const s = readFileSync(f, "utf8");
      return [...s.matchAll(MUTATION)].map((m) => ({
        file: relative(ROOT, f),
        src: s,
        at: m.index ?? 0,
      }));
    });
    expect(sites.length).toBeGreaterThanOrEqual(8);
    const bad = sites
      .filter(({ src, at }) => {
        const win = src.slice(Math.max(0, at - 600), at + 600);
        return !/rawFetchHelpers\.csrf|csrfHeaderAsync/.test(win);
      })
      .map((s) => s.file);
    expect(bad).toEqual([]);
  });
});
