import { describe, it, expect, vi, beforeEach, afterEach } from "vitest";
import { render, screen, waitFor, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";

import { PollWidget } from "@/components/forum-poll";
import { BountyAcceptButton } from "@/components/forum-vote";
import { FunTab } from "@/components/content-manage-tabs";
import { toast } from "@/components/ui/toast";
import { LocaleProvider } from "@/i18n/client";
import { zhCN } from "@/i18n/zh-CN";

/**
 * 写操作失败必须有声音（批四 B2）。
 *
 * 这批站点原先全是 `catch {}` / `.catch(() => {})`：请求被后端拒绝后按钮
 * 转一圈回到原样，界面既不报错也不说明为什么——用户只知道自己「点了没反应」，
 * 版主还会以为是自己权限不够。三例各钉一条：失败时把后端那句中文判据
 * 原样送回界面（toast 或 role="alert"），而不是演成「什么都没发生」。
 *
 * 负控：把这三处换回旧写法（空 catch）后本文件三条断言必须全红。
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

/** 一律回业务失败：message 是后端已经本地化好的判据原文 */
function stubFail(message: string) {
  const spy = vi.fn(async () =>
    res({ code: 9999, message, data: null, request_id: "t" }),
  );
  vi.stubGlobal("fetch", spy);
  return spy;
}

const POLL = {
  options: ["甲", "乙"],
  closed: false,
  my_vote: null,
  total: 3,
  counts: [
    { index: 0, votes: 2 },
    { index: 1, votes: 1 },
  ],
};

const FUN = [
  {
    id: 9,
    username: "阿宝",
    title: "今日梗图",
    body: null,
    status: "normal",
    added: "2026-10-17T00:00:00Z",
    fun_votes: 1,
    dull_votes: 0,
    my_vote: null,
  },
];

let errSpy: ReturnType<typeof vi.spyOn>;

beforeEach(() => {
  errSpy = vi.spyOn(toast, "error").mockImplementation(() => 0);
});

afterEach(() => {
  vi.restoreAllMocks();
  vi.unstubAllGlobals();
});

async function confirmDanger(title: string) {
  const dlg = await screen.findByRole("alertdialog", { name: title });
  await userEvent.click(
    within(dlg).getByRole("button", { name: zhCN.ui.confirmDelete }),
  );
}

function mount(node: React.ReactNode) {
  return render(
    <LocaleProvider dict={zhCN} locale="zh-CN">
      {node}
    </LocaleProvider>,
  );
}

describe("写失败必须出声", () => {
  it("投票截止失败：面板里报出后端判据，不演「已截止」", async () => {
    stubFail("投票已截止，不能再改");
    mount(<PollWidget topicId={1} poll={POLL} canClose />);
    await userEvent.click(
      await screen.findByRole("button", { name: zhCN.forums.pollClose }),
    );
    await confirmDanger(zhCN.forums.pollClose);
    await waitFor(() =>
      expect(screen.getByRole("alert").textContent).toContain(
        "投票已截止，不能再改",
      ),
    );
  });

  it("悬赏采纳失败：toast 说清为什么没发出去", async () => {
    stubFail("赏金已经发完了");
    mount(
      <BountyAcceptButton topicId={1} postId={2} spark={50} currency="馒头" />,
    );
    await userEvent.click(
      // 可及名含金额后缀（「采纳 +50 馒头」），按前缀取
      await screen.findByRole("button", {
        name: new RegExp("^" + zhCN.forums.bountyAccept),
      }),
    );
    await confirmDanger(zhCN.forums.bountyAccept);
    await waitFor(() =>
      expect(errSpy).toHaveBeenCalledWith("赏金已经发完了"),
    );
  });

  it("趣味盒禁止失败：toast 出声（父级 flash 只管读，不背写的锅）", async () => {
    stubFail("趣味内容不存在");
    mount(
      <FunTab fun={FUN} load={() => undefined} t={zhCN.cmgmt} />,
    );
    await userEvent.click(
      await screen.findByRole("button", { name: zhCN.cmgmt.btnBan }),
    );
    await waitFor(() => expect(errSpy).toHaveBeenCalledWith("趣味内容不存在"));
  });
});
