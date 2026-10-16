import { describe, it, expect, vi } from "vitest";
import {
  render,
  screen,
  waitFor,
  within,
} from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { useState } from "react";

import {
  UserDetailDialogs,
  type AskAct,
} from "@/components/admin-user-detail-dialogs";
import { LocaleProvider } from "@/i18n/client";
import { zhCN } from "@/i18n/zh-CN";

/**
 * 删用户确认必须「打名字才放行」（批五 C1 之一）。
 *
 * `DELETE /admin/users/{id}` 是一条约 50 张表的物理清理（站内信/帖子/勋章/
 * 角色全删，种子与公告的作者置 NULL），而它原先只有一句话确认框，message 里
 * 同时给了名字和 uid——看进去的是 uid、点下去的是按钮，撞名或翻页面时点错
 * 相邻用户就没了。这里钉住：没原样输入用户名时确认钮禁着，输对才交付一次
 * onAct("deleteUser")。
 */

const NAME = "阿宝";

function Harness({ onAct }: { onAct: (a: AskAct) => void }) {
  const [askAct, setAskAct] = useState<AskAct | null>(null);
  const [askStatus, setAskStatus] = useState<0 | 1 | 2 | null>(null);
  const [statusReason, setStatusReason] = useState("");
  const [newName, setNewName] = useState("");
  return (
    <>
      <button type="button" onClick={() => setAskAct("deleteUser")}>
        开删除
      </button>
      <button type="button" onClick={() => setAskAct("resetPass")}>
        开重置
      </button>
      <UserDetailDialogs
        askStatus={askStatus}
        setAskStatus={setAskStatus}
        statusReason={statusReason}
        setStatusReason={setStatusReason}
        askAct={askAct}
        setAskAct={setAskAct}
        username={NAME}
        newName={newName}
        uid={4242}
        onStatus={() => undefined}
        onAct={onAct}
      />
    </>
  );
}

function mount(onAct: (a: AskAct) => void) {
  return render(
    <LocaleProvider dict={zhCN} locale="zh-CN">
      <Harness onAct={onAct} />
    </LocaleProvider>,
  );
}

describe("删用户二次输入", () => {
  it("没打对用户名时确认钮禁着，打对才交付一次", async () => {
    const onAct = vi.fn();
    mount(onAct);
    await userEvent.click(screen.getByRole("button", { name: "开删除" }));
    const dlg = await screen.findByRole("alertdialog");
    const input = within(dlg).getByRole("textbox");
    const ok = within(dlg).getByRole("button", {
      name: zhCN.ui.confirmDelete,
    });
    expect(ok).toBeDisabled();
    await userEvent.type(input, "阿");
    expect(ok).toBeDisabled();
    expect(onAct).not.toHaveBeenCalled();
    await userEvent.clear(input);
    await userEvent.type(input, NAME);
    await waitFor(() => expect(ok).toBeEnabled());
    await userEvent.click(ok);
    expect(onAct).toHaveBeenCalledTimes(1);
    expect(onAct).toHaveBeenCalledWith("deleteUser");
  });

  it("重置密码不要打名字（护栏只压在不可逆那条上）", async () => {
    const onAct = vi.fn();
    mount(onAct);
    // 同一个对话框组件按 askAct 分流：resetPass 分支必须没有输入槽
    await userEvent.click(screen.getByRole("button", { name: "开重置" }));
    const dlg = await screen.findByRole("alertdialog");
    expect(within(dlg).queryByRole("textbox")).toBeNull();
    await userEvent.click(
      within(dlg).getByRole("button", { name: zhCN.ui.confirmOk }),
    );
    expect(onAct).toHaveBeenCalledWith("resetPass");
  });
});
