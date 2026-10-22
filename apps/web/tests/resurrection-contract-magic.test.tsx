import { describe, it, expect, vi, afterEach } from "vitest";
import { render, screen, waitFor } from "@testing-library/react";

import { ResurrectionPanel } from "@/components/resurrection-panel";
import { LocaleProvider } from "@/i18n/client";
import { zhCN } from "@/i18n/zh-CN";

/**
 * P-558 渲染层证据（保种区契约条——「折成空串」那一档）。
 *
 * 同一枚 `{magic}`，两种坏法：上一支用例治的是**露出字面花括号**，
 * 这里治的是**走了 fmt 却没喂 magic** ⇒ `fmt()` 把占位折成空串，
 * 界面显示「需做种 24 小时 → 500  + 免费券」（数字后面凭空缺一块，
 * 用户不知道 500 是什么的单位）。所以断言要同时钉两头：货币名在、
 * 且没有「数字 + 两个空格」这种折空形状。
 */

const ROW = {
  torrent_id: 51001,
  name: "旧片·待救",
  size: 1073741824,
  required_hours: 24,
  reward_sparks: 500,
};

function res(body: unknown): Response {
  return {
    ok: true,
    status: 200,
    headers: { get: () => "application/json" },
    json: async () => body,
  } as unknown as Response;
}

afterEach(() => vi.unstubAllGlobals());

describe("保种契约条的货币名（渲染层）", () => {
  it("契约句里货币名要在，且不许出现被折空的缺口", async () => {
    vi.stubGlobal(
      "fetch",
      vi.fn(async (input: unknown) => {
        const url = String(input);
        if (url.includes("/resurrections/mine")) {
          return res({ code: 0, message: "ok", data: [] });
        }
        return res({ code: 0, message: "ok", data: [ROW] });
      }),
    );
    const { container } = render(
      <LocaleProvider dict={zhCN} locale="zh-CN" currency="魔力">
        <ResurrectionPanel />
      </LocaleProvider>,
    );
    await waitFor(() =>
      expect(container.textContent).toContain("旧片·待救"),
    );
    const txt = container.textContent ?? "";
    expect(txt).toContain("需做种 24 小时 → 500 魔力");
    expect(txt).not.toContain("{magic}");
    expect(txt).not.toContain("500  +");
    expect(screen.getAllByText(/契约/).length).toBeGreaterThan(0);
  });
});
