import { describe, it, expect } from "vitest";
import { render, screen, fireEvent } from "@testing-library/react";
import userEvent from "@testing-library/user-event";

import { useConfirm, AskDialog } from "@/components/ui/use-confirm";
import { LocaleProvider } from "@/i18n/client";
import { zhCN } from "@/i18n/zh-CN";

/**
 * 确认状态机的两个新槽（2026-10-16 弹窗普查）。
 *
 *  - requireText：删除类动作要「原样输入才解禁确认钮」。这能力原本只有直挂
 *    ConfirmDialog 的调用点用得上（全站 1 处），走 ask 状态机的全用不上——
 *    而危险度最高的那几个后台动作恰恰都走状态机。
 *  - check：一个复选槽。申诉受理要同时回答「是否联动解封」；旧写法是提交
 *    accept 之后再弹第二个框，后端 CAS（`WHERE status='open'`）必拒第二次
 *    ⇒ 那一步永远执行不到。同框一次提交才是真确认。
 */

function Harness({
  requireText,
  check,
  onRun,
}: {
  requireText?: string;
  check?: { label: string; initial?: boolean };
  onRun: (text: string, checked: boolean) => void;
}) {
  const ask = useConfirm();
  return (
    <LocaleProvider dict={zhCN} locale="zh-CN">
      <button
        type="button"
        onClick={() => {
          ask.open({
            title: "删除被禁账号",
            message: "不可恢复",
            tone: "danger",
            requireText,
            check,
            label: check ? "批注" : undefined,
            run: onRun,
          });
        }}
      >
        触发
      </button>
      <AskDialog {...ask} />
    </LocaleProvider>
  );
}

describe("useConfirm 的 requireText 与 check 槽", () => {
  it("requireText 没原样输入时确认钮禁着，输对才放行", async () => {
    const runs: [string, boolean][] = [];
    render(<Harness requireText="1497" onRun={(t, c) => runs.push([t, c])} />);
    await userEvent.click(screen.getByRole("button", { name: "触发" }));
    const okBtn = screen.getByRole("button", { name: zhCN.ui.confirmDelete });
    const input = screen.getByRole("textbox");
    expect(okBtn).toBeDisabled();
    await userEvent.type(input, "149");
    expect(okBtn).toBeDisabled();
    // 差一位都不许（少一个数量级就是少删一千个账号）
    await userEvent.clear(input);
    await userEvent.type(input, "14970");
    expect(okBtn).toBeDisabled();
    await userEvent.clear(input);
    await userEvent.type(input, " 1497 ");
    // 首尾空白按 ConfirmDialog 既有口径 trim 掉，算输对
    expect(okBtn).not.toBeDisabled();
    await userEvent.click(okBtn);
    // requireText 那格输入是 ConfirmDialog 的**内部**状态（只用来解禁确认钮），
    // 不进 run 的 text——text 是 prompt 槽（label）的值，这里没开 prompt 槽。
    // 记下来是因为两者可以同框共存，调用方别指望 run 能收到用户输入的那串数。
    expect(runs).toEqual([["", false]]);
  });

  it("check 槽把勾选状态带回 run；initial=true 时默认勾着", async () => {
    const seen: boolean[] = [];
    render(
      <Harness
        check={{ label: "同时解除该账号的封禁？", initial: true }}
        onRun={(_t, c) => seen.push(c)}
      />,
    );
    await userEvent.click(screen.getByRole("button", { name: "触发" }));
    const box = screen.getByRole("checkbox") as HTMLInputElement;
    expect(box.checked).toBe(true);
    // 默认解是「勾着就解封」——申诉受理的正路
    fireEvent.click(
      screen.getByRole("button", { name: zhCN.ui.confirmDelete }),
    );
    expect(seen).toEqual([true]);

    // 取消勾选＝只受理不解封，仍然是一次提交（不再有第二个框）
    seen.length = 0;
    await userEvent.click(screen.getByRole("button", { name: "触发" }));
    await userEvent.click(screen.getByRole("checkbox"));
    expect(
      screen.getByRole("checkbox") as HTMLInputElement,
    ).not.toBeChecked();
    fireEvent.click(
      screen.getByRole("button", { name: zhCN.ui.confirmDelete }),
    );
    expect(seen).toEqual([false]);
  });

  it("没有 check 槽时不渲染 checkbox（旧调用点形状不变）", async () => {
    render(<Harness onRun={() => undefined} />);
    await userEvent.click(screen.getByRole("button", { name: "触发" }));
    expect(screen.queryByRole("checkbox")).toBeNull();
  });
});
