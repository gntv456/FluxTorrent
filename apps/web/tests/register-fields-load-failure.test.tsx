import { describe, it, expect, vi, beforeEach } from "vitest";
import { render, screen, waitFor } from "@testing-library/react";

import RegisterPage from "@/app/(auth)/register/page";
import { LocaleProvider } from "@/i18n/client";
import { zhCN } from "@/i18n/zh-CN";

/**
 * P-618：注册页把 GET /register-fields 的失败演成「本站没有自定义字段」。
 * 站长标记为必填的字段随之从表单消失，提交必被后端以「「X」为必填字段」
 * 拒绝，而会员在页面上根本找不到那个字段。负控回放 HEAD 的两份文件
 * （页面 + 渲染件）⇒ 第一条用例红在「找不到加载失败提示」。
 */

const { get } = vi.hoisted(() => ({ get: vi.fn() }));

vi.mock("next/navigation", () => ({
  useRouter: () => ({ push: vi.fn(), replace: vi.fn(), refresh: vi.fn() }),
}));

vi.mock("@/lib/api-client", async (importOriginal) => {
  const actual = await importOriginal<typeof import("@/lib/api-client")>();
  return {
    ...actual,
    api: { get, post: vi.fn() },
    setSessionCookie: vi.fn(),
  };
});

function route(url: string) {
  if (url === "/api/v1/register-mode") {
    return Promise.resolve({ mode: "open", captcha_provider: "none" });
  }
  if (url === "/api/v1/auth/captcha") {
    return Promise.resolve({ captcha_id: "c1", question: "1+1" });
  }
  return Promise.reject(new Error(`unexpected GET ${url}`));
}

/** register-fields 口：payload 为 Error 时拒绝，否则按给定 data 回包 */
function stubFields(payload: unknown) {
  vi.stubGlobal(
    "fetch",
    vi.fn(async (url: string) => {
      const isFields = String(url).includes("/api/v1/register-fields");
      if (isFields && payload instanceof Error) throw payload;
      return {
        json: async () => ({ data: isFields ? payload : [] }),
      };
    }),
  );
}

function mount() {
  render(
    <LocaleProvider dict={zhCN} locale="zh-CN">
      <RegisterPage />
    </LocaleProvider>,
  );
}

beforeEach(() => {
  get.mockReset();
  get.mockImplementation(route);
});

describe("P-618 注册自定义字段取数失败", () => {
  it("失败时给出加载失败提示，表单其余部分仍可用", async () => {
    stubFields(new Error("boom"));
    mount();
    const alert = await screen.findByRole("alert");
    expect(alert.textContent).toContain(zhCN.common.loadFailed);
    // 其余注册表单不能被整页打空
    expect(
      screen.getByRole("button", { name: zhCN.register.submit }),
    ).toBeTruthy();
  });

  it("字段清单成功下发时正常渲染（正控）", async () => {
    stubFields([
      {
        key: "qq",
        label: "QQ",
        type: "text",
        required: true,
        options: [],
      },
    ]);
    mount();
    expect(await screen.findByText(/QQ/)).toBeTruthy();
    expect(screen.queryByText(zhCN.common.loadFailed)).toBeNull();
  });

  it("真实空清单不报加载失败", async () => {
    stubFields([]);
    mount();
    await waitFor(() =>
      expect(screen.queryByText(zhCN.common.loadFailed)).toBeNull(),
    );
  });
});
