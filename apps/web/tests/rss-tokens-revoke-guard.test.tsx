/**
 * 撤销守卫回归测试（复查 P1）：撤销当前生效 token 后，
 * 顶部订阅链接必须清空——守卫依赖 plain 字段被 create/rotate 正确赋值。
 */
/** 撤销后清链接的复用片段：mock 面板 + 模拟 create→revoke 全流程 */
import { describe, expect, it, vi, beforeEach } from "vitest";
import { render, screen, waitFor } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { RssTokenPanel } from "@/components/rss-tokens";

const apiGet = vi.hoisted(() => vi.fn());
const apiPost = vi.hoisted(() => vi.fn());
const apiDel = vi.hoisted(() => vi.fn());

vi.mock("@/lib/api-client", () => ({
  api: {
    get: apiGet,
    post: apiPost,
    del: apiDel,
  },
}));

vi.mock("next/navigation", () => ({
  useRouter: () => ({ replace: vi.fn() }),
  useSearchParams: () => new URLSearchParams(),
}));

const dict = {
  getrss: {
    tokenTitle: "订阅令牌",
    tokenNote: "note",
    tokenNamePh: "name",
    tokenCreate: "创建",
    tokenRotate: "轮换",
    tokenRevoke: "撤销",
    tokenEmpty: "empty",
    tokenPlainGone: "明文已不可取",
    copy: "复制",
    copied: "已复制",
    tokenRequired: "需要先创建订阅令牌",
  },
};

vi.mock("@/i18n/client", () => ({
  useI18n: () => ({ dict, t: (k: string) => k }),
}));

describe("撤销 token 后清空订阅链接（P1 回归）", () => {
  beforeEach(() => {
    vi.clearAllMocks();
  });

  it("create 后撤销：onPlain('') 必须被调用、行内明文消失", async () => {
    apiPost.mockResolvedValueOnce({
      id: 101,
      name: "t1",
      created_at: "2026-11-05T00:00:00Z",
      token: "fxr_probe123",
      hint: "h",
    });
    apiDel.mockResolvedValueOnce({ ok: true });

    const onPlain = vi.fn();
    const user = userEvent.setup();
    render(
      <RssTokenPanel
        base="http://x/api/v1/rss/"
        initial={[]}
        query=""
        onPlain={onPlain}
      />,
    );

    // 创建 → onPlain 收到明文
    await user.type(screen.getByPlaceholderText("name"), "t1");
    await user.click(screen.getByRole("button", { name: "创建" }));
    await waitFor(() =>
      expect(onPlain).toHaveBeenLastCalledWith("fxr_probe123"),
    );

    // 行内明文 URL 可见（plain 已接上——修复前这里是「明文已不可取」）
    expect(
      (screen.getByDisplayValue(/fxr_probe123/) as HTMLInputElement).value,
    ).toContain("fxr_probe123");

    // 撤销 → onPlain("") 必须触发（修复前守卫是死代码，永不触发）
    await user.click(screen.getByRole("button", { name: "撤销" }));
    await waitFor(() => expect(onPlain).toHaveBeenLastCalledWith(""));
    expect(screen.queryByDisplayValue(/fxr_probe123/)).toBeNull();
    expect(screen.getByText("empty")).toBeDefined();
  });

  it("轮换后：新行内明文接上新 token（plain 跟随新响应）", async () => {
    apiPost.mockResolvedValueOnce({
      id: 202,
      name: "t2",
      created_at: "2026-11-05T00:00:00Z",
      token: "fxr_rotated999",
      hint: "h",
    });

    const onPlain = vi.fn();
    const user = userEvent.setup();
    render(
      <RssTokenPanel
        base="http://x/api/v1/rss/"
        initial={[
          {
            id: 201,
            name: "t2",
            created_at: "2026-11-05T00:00:00Z",
            last_used_at: null,
          },
        ]}
        query=""
        onPlain={onPlain}
      />,
    );

    await user.click(screen.getAllByRole("button", { name: "轮换" })[0]);
    await waitFor(() =>
      expect(onPlain).toHaveBeenLastCalledWith("fxr_rotated999"),
    );
    // 旧行（201）被新行（202）替换，行内显示新明文
    expect(screen.getByDisplayValue(/fxr_rotated999/)).toBeDefined();
  });
});
