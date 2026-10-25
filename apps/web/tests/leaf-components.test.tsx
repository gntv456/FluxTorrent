import { describe, it, expect, vi, beforeEach } from "vitest";
import { render, screen, waitFor } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { MedalActions } from "@/components/medal-actions";
import { ClaimButton } from "@/components/claim-button";

/** 交互叶子组件主链路：勋章购买/赠送、保种认领 —— mock api，断言请求与 UI 反馈 */

function ok(data: unknown = {}) {
  return Promise.resolve(data);
}

beforeEach(() => {
  vi.restoreAllMocks();
});

describe("MedalActions", () => {
  it("未拥有 → 点击购买调 /medals/buy 并转已拥有", async () => {
    const spy = vi.fn().mockReturnValue(ok({}));
    vi.stubGlobal(
      "fetch",
      vi.fn().mockResolvedValue({
        ok: true,
        headers: new Headers({ "content-type": "application/json" }),
        json: () => Promise.resolve({ code: 0, message: "ok", data: spy() }),
      }),
    );
    // 直接 spy api 模块更稳
    const { api } = await import("@/lib/api-client");
    const buySpy = vi
      .spyOn(api, "post")
      .mockImplementation(() => ok({}) as never);
    render(
      <MedalActions
        medalId={5}
        owned={false}
        wearing={false}
        price={1000}
        getType={1}
      />,
    );
    await userEvent.click(screen.getByRole("button", { name: /购买/ }));
    await waitFor(() =>
      expect(buySpy).toHaveBeenCalledWith(
        "/api/v1/medals/buy",
        expect.objectContaining({ medal_id: 5 }),
      ),
    );
  });

  it("已拥有 → 赠送走输入槽对话框，to_user 取自输入", async () => {
    const { api } = await import("@/lib/api-client");
    const giftSpy = vi
      .spyOn(api, "post")
      .mockImplementation(() => ok({}) as never);
    render(
      <MedalActions
        medalId={5}
        owned={true}
        wearing={false}
        price={1000}
        getType={1}
      />,
    );
    await userEvent.click(screen.getByRole("button", { name: /赠送/ }));
    // P2 清偿：window.prompt 退役 → ConfirmDialog 输入槽
    const input = await screen.findByLabelText(/赠送给/, {
      selector: "input",
    });
    await userEvent.type(input, "friend");
    await userEvent.click(screen.getByRole("button", { name: /^确认$|^OK$/ }));
    await waitFor(() =>
      expect(giftSpy).toHaveBeenCalledWith("/api/v1/medals/gift", {
        medal_id: 5,
        to_user: "friend",
      }),
    );
    expect(await screen.findByText(/已赠送给 friend/)).toBeInTheDocument();
  });

  it("购买失败显示错误消息", async () => {
    const { api, ApiError } = await import("@/lib/api-client");
    vi.spyOn(api, "post").mockRejectedValue(
      new ApiError(3001, "余额不足") as never,
    );
    render(
      <MedalActions
        medalId={5}
        owned={false}
        wearing={false}
        price={1000}
        getType={1}
      />,
    );
    await userEvent.click(screen.getByRole("button", { name: /购买/ }));
    expect(await screen.findByText(/余额不足/)).toBeInTheDocument();
  });

  it("授予型（get_type=2）即便带价格也不给购买按钮", () => {
    // 回归锚点：库里存在 get_type=2 却 price=30000 的勋章（保种达人类），
    // 旧口径按 price 判断会渲染出点不动的「购买」。
    render(
      <MedalActions
        medalId={5}
        owned={false}
        wearing={false}
        price={30000}
        getType={2}
      />,
    );
    expect(screen.queryByRole("button", { name: /购买/ })).toBeNull();
    // 锁定态是静态 chip（非可点按钮）——按文本断言，不按 role=button
    expect(screen.getByText(/仅授予/)).toBeInTheDocument();
  });

  it("余额不足 → 按钮禁用并显示「余额不足」，不发起购买", async () => {
    const { api } = await import("@/lib/api-client");
    const buySpy = vi
      .spyOn(api, "post")
      .mockImplementation(() => ok({}) as never);
    render(
      <MedalActions
        medalId={5}
        owned={false}
        wearing={false}
        price={30000}
        getType={1}
        balance={100}
      />,
    );
    const btn = screen.getByRole("button", {
      name: /余额不足/,
    }) as HTMLButtonElement;
    expect(btn).toBeDisabled();
    await userEvent.click(btn);
    expect(buySpy).not.toHaveBeenCalled();
  });

  it("余额未知（null）不预判，按钮可点交后端校验", () => {
    render(
      <MedalActions
        medalId={5}
        owned={false}
        wearing={false}
        price={30000}
        getType={1}
        balance={null}
      />,
    );
    const btn = screen.getByRole("button", {
      name: /购买/,
    }) as HTMLButtonElement;
    expect(btn).toBeEnabled();
  });
});

describe("ClaimButton", () => {
  it("点击认领调 /preserve/claim 并转完成态", async () => {
    const { api } = await import("@/lib/api-client");
    const spy = vi.spyOn(api, "post").mockImplementation(() => ok({}) as never);
    render(
      <ClaimButton
        torrentId={9}
        claimed={false}
        label="认领"
        claimedLabel="已认领"
      />,
    );
    await userEvent.click(screen.getByRole("button", { name: "认领" }));
    await waitFor(() =>
      expect(spy).toHaveBeenCalledWith("/api/v1/preserve/claim", {
        torrent_id: 9,
      }),
    );
    expect(await screen.findByText("已认领")).toBeInTheDocument();
  });

  it("已认领初始即渲染完成态（无按钮）", () => {
    render(
      <ClaimButton
        torrentId={9}
        claimed={true}
        label="认领"
        claimedLabel="已认领"
      />,
    );
    expect(screen.getByText("已认领")).toBeInTheDocument();
    expect(screen.queryByRole("button")).toBeNull();
  });
});
