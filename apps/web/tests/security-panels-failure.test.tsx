import { describe, it, expect, vi, afterEach } from "vitest";
import { render, screen } from "@testing-library/react";

import { SessionsCard } from "@/components/sessions-card";
import { LoginHistory } from "@/components/login-history";
import { LocaleProvider } from "@/i18n/client";
import { zhCN } from "@/i18n/zh-CN";

/** 账号安全面板的两块：取数失败必须报「加载失败」，
 *  不能演成「近 24 小时无登录记录」/「暂无登录记录」——
 *  那会让来查异常登录的会员误以为账号干净。 */

afterEach(() => vi.unstubAllGlobals());

function failAll() {
  vi.stubGlobal(
    "fetch",
    vi.fn(async () => {
      throw new Error("network down");
    }),
  );
}

function mount(node: React.ReactNode) {
  return render(
    <LocaleProvider dict={zhCN} locale="zh-CN">{node}</LocaleProvider>,
  );
}

describe("安全面板失败不演空态", () => {
  it("活跃会话：失败报加载失败，不报「无登录记录」", async () => {
    failAll();
    mount(<SessionsCard />);
    expect(await screen.findByText(zhCN.common.loadFailed)).toBeTruthy();
    expect(screen.queryByText(zhCN.sessions.empty)).toBeNull();
  });

  it("登录历史：失败报加载失败，不报「暂无登录记录」", async () => {
    failAll();
    mount(<LoginHistory />);
    expect(await screen.findByText(zhCN.common.loadFailed)).toBeTruthy();
    expect(
      screen.queryByText(zhCN.usercp.security.loginsEmpty),
    ).toBeNull();
  });
});
