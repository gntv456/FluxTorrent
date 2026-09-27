import { describe, it, expect, vi, beforeEach } from "vitest";
import { render, screen, waitFor } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { SetupStepSite, type SiteDraft } from "@/app/setup/_parts/step-site";
import type { Status } from "@/app/setup/_parts/setup-types";

/**
 * G12 复验发现的回归闸门：预置 root 仍是临时密码时（root_temp_password），
 * must_reset 闸门拦掉 POST /setup —— 第二步必须就地改密并把新口令交回向导，
 * 否则站长在最后一步无路可走。此用例锁住「就地改密」这条通路。
 */

const blank: SiteDraft = {
  siteName: "",
  username: "",
  password: "",
  announceUrl: "",
};

function statusOf(rootTemp: boolean): Status {
  return {
    done: false,
    has_admin: true,
    root_temp_password: rootTemp,
    site_name: "FluxTorrent",
    packs: [],
  };
}

const t = (_k: string, fallback: string) => fallback;
const btn = "btn";

beforeEach(() => {
  vi.restoreAllMocks();
});

describe("SetupStepSite", () => {
  it("临时密码：渲染改密块，登录后调改密端点并以新口令回调", async () => {
    const { api } = await import("@/lib/api-client");
    const post = vi
      .spyOn(api, "post")
      .mockImplementation(() => Promise.resolve({}) as never);
    const onNext = vi.fn();
    render(
      <SetupStepSite
        t={t}
        btn={btn}
        status={statusOf(true)}
        initial={blank}
        onBack={() => {}}
        onNext={onNext}
      />,
    );
    expect(screen.getByText(/先设置新密码再继续/)).toBeTruthy();
    const user = userEvent.setup();
    await user.type(screen.getByLabelText(/管理员用户名/), "root");
    await user.type(
      screen.getByLabelText(/管理员密码$|^管理员密码/),
      "password123",
    );
    // 新口令短于 8 位 → 本地拦截，不发请求
    await user.type(screen.getByLabelText(/新密码（至少 8 位）/), "short");
    await user.type(screen.getByLabelText(/确认新密码/), "short");
    await user.click(screen.getByRole("button", { name: /下一步/ }));
    expect(screen.getByText(/新密码至少 8 位/)).toBeTruthy();
    expect(post).not.toHaveBeenCalled();
    expect(onNext).not.toHaveBeenCalled();

    // 两次不一致 → 仍拦截（先都补到 8 位以上，避免撞「太短」分支）
    await user.clear(screen.getByLabelText(/新密码（至少 8 位）/));
    await user.type(screen.getByLabelText(/新密码（至少 8 位）/), "newpass123");
    await user.clear(screen.getByLabelText(/确认新密码/));
    await user.type(screen.getByLabelText(/确认新密码/), "another88");
    await user.click(screen.getByRole("button", { name: /下一步/ }));
    expect(screen.getByText(/两次输入的新密码不一致/)).toBeTruthy();
    expect(post).not.toHaveBeenCalled();

    // 合法 → login → 改密 → 回调新口令
    await user.clear(screen.getByLabelText(/确认新密码/));
    await user.type(screen.getByLabelText(/确认新密码/), "newpass123");
    await user.click(screen.getByRole("button", { name: /下一步/ }));
    await waitFor(() => expect(onNext).toHaveBeenCalledTimes(1));
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

  it("非临时密码：无改密块，原样交回草稿且不发请求", async () => {
    const { api } = await import("@/lib/api-client");
    const post = vi
      .spyOn(api, "post")
      .mockImplementation(() => Promise.resolve({}) as never);
    const onNext = vi.fn();
    render(
      <SetupStepSite
        t={t}
        btn={btn}
        status={statusOf(false)}
        initial={blank}
        onBack={() => {}}
        onNext={onNext}
      />,
    );
    expect(screen.queryByLabelText(/确认新密码/)).toBeNull();
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
