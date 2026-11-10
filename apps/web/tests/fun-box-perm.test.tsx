import { describe, it, expect, vi } from "vitest";
import { render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";

import { FunBox } from "@/components/fun-box";
import { LocaleProvider } from "@/i18n/client";
import { zhCN } from "@/i18n/zh-CN";

/**
 * 趣味盒按钮按身份显隐（2026-11-10 收口）。
 *
 * 后端写口守卫（作者或 FUN_MANAGE 可编辑/删除；禁判仅 FUN_MANAGE）一直
 * 是齐的，但前台按钮无条件渲染——普通用户界面上看到编辑/禁止/删除俱全，
 * 点了才吃 403，站长截图像「人人可操作」。列表口补 mine/can_moderate
 * 行级权限位后，按钮显隐与写口守卫同一把尺。
 *
 * 负控：把 fun-box.tsx 的 `(current.mine || current.can_moderate) &&`
 * 拿掉，下面「看不到」两条断言即红。
 */

vi.mock("next/navigation", () => ({
  useRouter: () => ({ refresh: vi.fn(), push: vi.fn(), replace: vi.fn() }),
}));

function res(body: unknown): Response {
  return {
    ok: true,
    status: 200,
    headers: { get: () => "application/json" },
    json: async () => body,
  } as unknown as Response;
}

const t = zhCN.funbox;

function item(o: {
  id: number;
  mine?: boolean;
  can_moderate?: boolean;
  status?: string;
}) {
  return {
    id: o.id,
    username: "阿宝",
    title: `条目${o.id}`,
    body: null,
    status: o.status ?? "normal",
    added: "2026-11-10T00:00:00Z",
    fun_votes: 1,
    dull_votes: 0,
    my_vote: null,
    mine: o.mine ?? false,
    can_moderate: o.can_moderate ?? false,
  };
}

function mount(items: ReturnType<typeof item>[]) {
  const payload = {
    code: 0,
    message: "ok",
    data: items,
    request_id: "t",
  };
  vi.stubGlobal("fetch", vi.fn(async () => res(payload)));
  return render(
    <LocaleProvider dict={zhCN} locale="zh-CN">
      <FunBox />
    </LocaleProvider>,
  );
}

async function actsArea() {
  return screen.findByText(`条目1`).then(() =>
    document.querySelector(".funbox__acts"),
  );
}

describe("趣味盒按钮按身份显隐", () => {
  it("别人的条目、普通访客：编辑/禁止/删除全不可见", async () => {
    mount([item({ id: 1 })]);
    const acts = await actsArea();
    expect(acts?.textContent ?? "").toBe("");
    // 投票按钮仍在（读侧动作不设限）
    expect(await screen.findByRole("button", { name: new RegExp(t.voteFun) }));
  });

  it("自己的条目：编辑/删除可见，禁止不可见（禁判是职员权限）", async () => {
    mount([item({ id: 1, mine: true })]);
    const acts = await actsArea();
    expect(acts?.textContent).toContain(t.edit);
    expect(acts?.textContent).toContain(t.delete);
    expect(acts?.textContent).not.toContain(t.ban);
  });

  it("职员（FUN_MANAGE）：编辑/禁止/删除全可见", async () => {
    mount([item({ id: 1, can_moderate: true })]);
    const acts = await actsArea();
    expect(acts?.textContent).toContain(t.edit);
    expect(acts?.textContent).toContain(t.ban);
    expect(acts?.textContent).toContain(t.delete);
  });

  it("历史列表同尺：普通访客对他人的历史条目无删除/禁止", async () => {
    mount([item({ id: 1, mine: true }), item({ id: 2 })]);
    // 展开「更多」
    await userEvent.click(
      await screen.findByRole("button", { name: new RegExp(t.more) }),
    );
    const li = document.querySelector(".funbox__history li:last-child");
    const acts = li?.querySelector(".funbox__acts");
    expect(acts?.textContent ?? "").toBe("");
  });
});
