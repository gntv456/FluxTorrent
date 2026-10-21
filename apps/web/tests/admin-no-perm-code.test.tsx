import { describe, it, expect, vi, afterEach } from "vitest";
import { render, screen } from "@testing-library/react";
import type { ReactNode } from "react";

import { LocaleProvider } from "@/i18n/client";
import { zhCN } from "@/i18n/zh-CN";
import { ReportBox } from "@/components/report-box";
import { CheaterBox } from "@/components/cheater-box";
import { CheatAppealQueue } from "@/components/cheat-appeal-queue";

/**
 * P-501：后端 Forbidden 走 HTTP 403，但信封 code 固定是 2003。
 * 三处队列此前都按 e.code === 403 判「无权限」，分支永远不命中：
 * 举报队列与作弊信箱把 403 演成「暂无记录」，申诉队列只剩通用错误。
 * 现网 A/B：root 200 → 用户级拒绝覆盖 403/2003 → 清除后 200。
 */

vi.mock("@/lib/api-client", async (importOriginal) => {
  const actual = await importOriginal<typeof import("@/lib/api-client")>();
  const deny = () =>
    Promise.reject(new actual.ApiError(2003, "无权限", null, 403, "t"));
  return { ...actual, api: { get: deny, post: deny } };
});

afterEach(() => {
  vi.clearAllMocks();
});

function mount(node: ReactNode) {
  return render(
    <LocaleProvider dict={zhCN} locale="zh-CN">
      {node}
    </LocaleProvider>,
  );
}

describe("管理队列的无权限态", () => {
  it("举报队列遇到 2003 显示无权限，而不是「暂无」", async () => {
    mount(<ReportBox />);
    expect(await screen.findByText(zhCN.reportbox.noPerm)).toBeTruthy();
    expect(screen.queryByText(zhCN.reportbox.queueEmpty)).toBeNull();
  });

  it("作弊信箱遇到 2003 显示无权限，而不是「暂无」", async () => {
    mount(<CheaterBox />);
    expect(await screen.findByText(zhCN.cheaterbox.noPerm)).toBeTruthy();
    expect(screen.queryByText(zhCN.cheaterbox.empty)).toBeNull();
  });

  it("申诉队列遇到 2003 显示无权限", async () => {
    mount(<CheatAppealQueue />);
    expect(await screen.findByText(zhCN.cheaterbox.appealNoPerm)).toBeTruthy();
  });
});
