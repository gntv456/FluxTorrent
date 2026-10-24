import { describe, it, expect, vi, beforeEach, afterEach } from "vitest";
import { render, screen, waitFor } from "@testing-library/react";
import userEvent from "@testing-library/user-event";

import {
  StaffEmailDomainsPanel,
} from "@/components/staff-tools-security-domains";
import { LocaleProvider } from "@/i18n/client";
import { fmt } from "@/i18n/config";
import { zhCN } from "@/i18n/zh-CN";

/**
 * P-638 续账的界面半：域名单株库「从文件导入」的接线。
 *
 * 后端文件口（multipart，不受 2MiB JSON 墙）已由 KS 闸门验过；但接口
 * 层绿 ≠ 站长用得上——文本框那条路数万条灌不进去。这条用例钉「面板里
 * 选文件真的发得出这个请求、并把后端的真实计数回报」；失败时服务端点名
 * 的「已导入前 N 条」也要原样上屏，不许折成通用网络错误。
 */

const { get } = vi.hoisted(() => ({ get: vi.fn() }));

vi.mock("@/lib/api-client", async (importOriginal) => {
  const actual = await importOriginal<typeof import("@/lib/api-client")>();
  return {
    ...actual,
    api: { get, post: vi.fn(), del: vi.fn() },
  };
});

const flash = vi.fn();
let calls: { url: string; init?: RequestInit }[] = [];
let importFail: string | null = null;

function resp(payload: unknown, ok = true) {
  return {
    ok,
    status: ok ? 200 : 400,
    json: async () => payload,
  } as unknown as Response;
}

beforeEach(() => {
  get.mockReset();
  get.mockResolvedValue({ items: [], total: 0 });
  flash.mockReset();
  calls = [];
  importFail = null;
  document.cookie = "flux_csrf=tok123; path=/";
  vi.stubGlobal(
    "fetch",
    vi.fn(async (input: RequestInfo | URL, init?: RequestInit) => {
      calls.push({ url: String(input), init });
      if (String(input).includes("/import-file")) {
        if (importFail) {
          return resp(
            { code: 1002, message: importFail, data: null, request_id: "t" },
            false,
          );
        }
        return resp({
          code: 0,
          data: {
            file: "list.txt",
            added: 2,
            skipped: 1,
            duplicates: 0,
            invalid: 0,
            total: 3,
            batches: 1,
            rejected: 0,
          },
          request_id: "t",
        });
      }
      return resp({ code: 0, data: null, request_id: "t" });
    }),
  );
});

afterEach(() => {
  vi.unstubAllGlobals();
  document.cookie = "flux_csrf=; path=/; max-age=0";
});

function mount() {
  return render(
    <LocaleProvider dict={zhCN} locale="zh-CN">
      <StaffEmailDomainsPanel flash={flash} />
    </LocaleProvider>,
  );
}

async function pickFile(container: HTMLElement, f: File) {
  await screen.findByText(zhCN.stafftools.edEmpty);
  const input = container.querySelector(
    'input[type="file"]',
  ) as HTMLInputElement;
  expect(input).not.toBeNull();
  // 先走真按钮：点「从文件导入」必须点到文件输入上（input.click）。
  // 直接 userEvent.upload 只触 change，按钮接死也照样绿。
  const spy = vi.spyOn(input, "click");
  await userEvent.click(
    screen.getByRole("button", { name: zhCN.stafftools.edFileBtn }),
  );
  expect(spy).toHaveBeenCalled();
  spy.mockRestore();
  await userEvent.upload(input, f);
  await waitFor(() => expect(calls.length).toBeGreaterThan(0));
}

describe("域名库文件导入接线（P-638 续账）", () => {
  it("选文件 → multipart POST 打到文件口 → flash 报后端计数", async () => {
    const { container } = mount();
    const f = new File(["a.invalid\nb.invalid\n"], "list.txt", {
      type: "text/plain",
    });
    await pickFile(container, f);
    expect(calls[0].url).toContain(
      "/api/v1/admin/email-domains/import-file",
    );
    expect(calls[0].init?.method).toBe("POST");
    expect(calls[0].init?.body).toBeInstanceOf(FormData);
    const body = calls[0].init?.body as FormData;
    expect(body.get("file")).toBe(f);
    await waitFor(() =>
      expect(flash).toHaveBeenCalledWith(
        fmt(zhCN.stafftools.edImported, {
          added: 2,
          skipped: 1,
          invalid: 0,
        }),
      ),
    );
  });

  it("部分提交的失败文案原样上屏，不折成通用网络错误", async () => {
    importFail = "域名库文件导入第 5 批失败（此前已提交 8000 条）：boom";
    const { container } = mount();
    await pickFile(container, new File(["x\n"], "big.txt"));
    await waitFor(() => expect(flash).toHaveBeenCalledWith(importFail));
    expect(flash).not.toHaveBeenCalledWith(zhCN.common.networkError);
  });
});