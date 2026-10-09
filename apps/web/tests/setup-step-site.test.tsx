import { describe, it, expect, vi, beforeEach } from "vitest";
import { render, screen, waitFor } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { SetupStepSite, type SiteDraft } from "@/app/setup/_parts/step-site";
import type { Status } from "@/app/setup/_parts/setup-types";

/**
 * 第二步「站点信息」的凭据区回归闸门。
 *
 * 背景（两轮缺陷）：
 * 1) G12：预置 root 仍是临时密码时 must_reset 闸门拦掉 POST /setup ——
 *    第二步必须就地改密并把新口令交回向导，否则站长在最后一步无路可走。
 * 2) 实测困惑：新旧两组密码框并列——上面「新密码/确认新密码」是设新，
 *    下面「管理员用户名/管理员密码」是填旧凭据登录，站长无从判断该填
 *    哪个，填错即「凭证无效」。而 needReset 时下面两个框要填的其实是
 *    系统已知常量（0017 引导的 root + password123），故收敛为：
 *    仍是默认口令 → 自动填、只让站长设新密码；已被重置 → 手填并说明。
 */

const blank: SiteDraft = {
  siteName: "",
  username: "",
  password: "",
  announceUrl: "",
};

function statusOf(
  rootTemp: boolean,
  opts: { user?: string; defaultPw?: boolean } = {},
): Status {
  return {
    done: false,
    has_admin: true,
    root_temp_password: rootTemp,
    temp_admin_username: opts.user ?? "root",
    temp_admin_default_pw: opts.defaultPw ?? true,
    site_name: "FluxTorrent",
    packs: [],
  };
}

const t = (_k: string, fallback: string) => fallback;
const btn = "btn";

function mount(status: Status, onNext = vi.fn()) {
  render(
    <SetupStepSite
      t={t}
      btn={btn}
      status={status}
      initial={blank}
      onBack={() => {}}
      onNext={onNext}
    />,
  );
  return onNext;
}

beforeEach(() => {
  vi.restoreAllMocks();
});

describe("SetupStepSite 凭据区", () => {
  it("仍是引导默认口令：账号/旧口令自动填，不渲染成输入框", async () => {
    const { api } = await import("@/lib/api-client");
    const post = vi
      .spyOn(api, "post")
      .mockImplementation(() => Promise.resolve({}) as never);
    const onNext = mount(statusOf(true));

    // 系统已知的凭据只读展示，不给「填错」的机会
    expect(screen.getByText("root")).toBeTruthy();
    expect(screen.queryByLabelText(/管理员用户名/)).toBeNull();
    expect(screen.queryByLabelText(/^管理员密码/)).toBeNull();

    const user = userEvent.setup();
    await user.type(screen.getByLabelText(/新密码（至少 8 位）/), "newpass123");
    await user.type(
      screen.getByLabelText(/再输一次新密码/),
      "newpass123",
    );
    await user.click(screen.getByRole("button", { name: /下一步/ }));
    await waitFor(() => expect(onNext).toHaveBeenCalledTimes(1));
    // 旧口令走的是自动填的 password123，不是站长手输
    expect(post).toHaveBeenNthCalledWith(1, "/api/v1/auth/login", {
      username: "root",
      password: "password123",
    });
    expect(post).toHaveBeenNthCalledWith(2, "/api/v1/me/password/change", {
      old_password: "password123",
      new_password: "newpass123",
    });
    expect(onNext.mock.calls[0][0]).toMatchObject({
      username: "root",
      password: "newpass123",
    });
  });

  it("新密码不足 8 位 / 两次不一致：本地拦截，不发请求", async () => {
    const { api } = await import("@/lib/api-client");
    const post = vi
      .spyOn(api, "post")
      .mockImplementation(() => Promise.resolve({}) as never);
    const onNext = mount(statusOf(true));
    const user = userEvent.setup();

    await user.type(screen.getByLabelText(/新密码（至少 8 位）/), "short");
    await user.type(screen.getByLabelText(/再输一次新密码/), "short");
    // 按钮 disabled（长度不足），点不动也发不出请求
    expect(
      screen.getByRole("button", { name: /下一步/ }).hasAttribute("disabled"),
    ).toBe(true);

    await user.clear(screen.getByLabelText(/新密码（至少 8 位）/));
    await user.type(screen.getByLabelText(/新密码（至少 8 位）/), "newpass123");
    await user.clear(screen.getByLabelText(/再输一次新密码/));
    await user.type(screen.getByLabelText(/再输一次新密码/), "another88");
    // 两次不一致 → 按钮同样 disabled（点「下一步」不触发 blur，
    // 这道校验必须在按钮态与提交路径上都在）
    expect(
      screen.getByRole("button", { name: /下一步/ }).hasAttribute("disabled"),
    ).toBe(true);
    expect(post).not.toHaveBeenCalled();
    expect(onNext).not.toHaveBeenCalled();
  });

  it("口令已被重置（非默认）：要手填当前密码，并说明它是哪一串", async () => {
    const { api } = await import("@/lib/api-client");
    const post = vi
      .spyOn(api, "post")
      .mockImplementation(() => Promise.resolve({}) as never);
    const onNext = mount(statusOf(true, { defaultPw: false }));

    // 不自动填了 → 当前密码框出现，且带解释
    expect(screen.getByLabelText(/当前密码/)).toBeTruthy();
    expect(screen.getByText(/重置密码时显示给你的那串/)).toBeTruthy();

    const user = userEvent.setup();
    await user.type(screen.getByLabelText(/当前密码/), "Tmp@abcd1234");
    await user.type(screen.getByLabelText(/新密码（至少 8 位）/), "newpass123");
    await user.type(screen.getByLabelText(/再输一次新密码/), "newpass123");
    await user.click(screen.getByRole("button", { name: /下一步/ }));
    await waitFor(() => expect(onNext).toHaveBeenCalledTimes(1));
    expect(post).toHaveBeenNthCalledWith(1, "/api/v1/auth/login", {
      username: "root",
      password: "Tmp@abcd1234",
    });
  });

  it("自动填不会反复触发（onChange 收敛，不死循环）", async () => {
    const status = statusOf(true);
    const { rerender } = render(
      <SetupStepSite
        t={t}
        btn={btn}
        status={status}
        initial={blank}
        onBack={() => {}}
        onNext={vi.fn()}
      />,
    );
    // 拿到自动填的值后反复重渲染：用户名非空 ⇒ patch 恒为空 ⇒ 不再 onChange
    await waitFor(() =>
      expect(
        screen.getByRole("button", { name: /下一步/ }).hasAttribute("disabled"),
      ).toBe(true),
    );
    for (let i = 0; i < 3; i += 1) {
      rerender(
        <SetupStepSite
          t={t}
          btn={btn}
          status={status}
          initial={blank}
          onBack={() => {}}
          onNext={vi.fn()}
        />,
      );
    }
    // 自动填后 root 账号在只读区可见，且没有多余输入框
    expect(screen.getAllByText("root")).toHaveLength(1);
    expect(screen.queryByLabelText(/管理员用户名/)).toBeNull();
  });

  it("已重置分支下旧口令填错：给出指路文案而非裸「凭证无效」", async () => {
    const { api, ApiError } = await import("@/lib/api-client");
    vi.spyOn(api, "post").mockImplementation((p: string) =>
      p === "/api/v1/auth/login"
        ? Promise.reject(new ApiError(2004, "凭证无效"))
        : (Promise.resolve({}) as never),
    );
    const onNext = mount(statusOf(true, { defaultPw: false }));
    const user = userEvent.setup();
    await user.type(screen.getByLabelText(/当前密码/), "wrong-guess");
    await user.type(screen.getByLabelText(/新密码（至少 8 位）/), "newpass123");
    await user.type(screen.getByLabelText(/再输一次新密码/), "newpass123");
    await user.click(screen.getByRole("button", { name: /下一步/ }));
    await waitFor(() =>
      expect(screen.getByText(/当前密码不正确/)).toBeTruthy(),
    );
    // 必须明确指向「改密前那串」，而不是让站长继续猜
    expect(screen.getByText(/改密前那串/)).toBeTruthy();
    expect(onNext).not.toHaveBeenCalled();
  });

  it("非临时密码：渲染普通登录区，无任何改密框", async () => {
    const { api } = await import("@/lib/api-client");
    const post = vi
      .spyOn(api, "post")
      .mockImplementation(() => Promise.resolve({}) as never);
    const onNext = mount(statusOf(false));

    expect(screen.queryByLabelText(/新密码/)).toBeNull();
    expect(screen.getByText(/用已有管理员账号登录/)).toBeTruthy();

    const user = userEvent.setup();
    await user.type(screen.getByLabelText(/管理员用户名/), "admin");
    await user.type(screen.getByLabelText(/^管理员密码/), "s3cretpw");
    await user.click(screen.getByRole("button", { name: /下一步/ }));
    expect(onNext).toHaveBeenCalledWith({
      siteName: "",
      username: "admin",
      password: "s3cretpw",
      announceUrl: "",
    });
    expect(post).not.toHaveBeenCalled();
  });
});
