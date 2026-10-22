import { describe, it, expect, vi, afterEach } from "vitest";
import { render, screen, waitFor } from "@testing-library/react";
import userEvent from "@testing-library/user-event";

import { RequestForm } from "@/components/request-board-form";
import { LocaleProvider } from "@/i18n/client";
import { zhCN } from "@/i18n/zh-CN";

/**
 * P-558 渲染层证据（求种区发布表单）。
 *
 * 站点货币名是后台可改的（`site_settings.currency_name`），文案里写的是
 * `{magic}` 占位；`fmt()` 对没喂值的占位一律折成空串，而这里两处**连 fmt
 * 都没走**——修复前用户看到的是字面「悬赏（{magic}）」这种带花括号的标签，
 * 提交回执同理。静态普查（scripts/audit_magic_placeholder.py）判出来之后，
 * 只有挂真组件渲染一遍才算数。
 */

function res(body: unknown): Response {
  return {
    ok: true,
    status: 200,
    headers: { get: () => "application/json" },
    json: async () => body,
  } as unknown as Response;
}

afterEach(() => vi.unstubAllGlobals());

function mount() {
  return render(
    <LocaleProvider dict={zhCN} locale="zh-CN" currency="魔力">
      <RequestForm onCreated={() => {}} onCancel={() => {}} />
    </LocaleProvider>,
  );
}

describe("求种表单的货币名（渲染层）", () => {
  it("标签与回执都补上货币名，界面上不许残留 {magic}", async () => {
    vi.stubGlobal(
      "fetch",
      vi.fn(async () => res({ code: 0, message: "ok", data: { id: 1 } })),
    );
    const { container } = mount();
    // 1) 未提交时：字段标签就是模板本身，修复前这里是「悬赏（{magic}）」
    expect(screen.getByText("悬赏（魔力）")).toBeInTheDocument();
    expect(container.textContent).not.toContain("{magic}");
    // 2) 提交成功后的回执：{n} 与 {magic} 都要有值
    await userEvent.type(screen.getByLabelText(/标题/), "求一部老片");
    await userEvent.click(screen.getByRole("button", { name: "发布求种" }));
    await waitFor(() =>
      expect(container.textContent).toContain(
        "发布成功！悬赏 100 魔力已冻结",
      ),
    );
    expect(container.textContent).not.toContain("{magic}");
    expect(container.textContent).not.toContain("悬赏 100 已冻结");
  });
});
