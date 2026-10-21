import { describe, it, expect, vi, beforeEach, afterEach } from "vitest";
import { render, screen, waitFor } from "@testing-library/react";
import userEvent from "@testing-library/user-event";

import { CommentLikeButton } from "@/components/comment-like-button";
import { LocaleProvider } from "@/i18n/client";
import { zhCN } from "@/i18n/zh-CN";
import { toast } from "@/components/ui/toast";

/**
 * 自赞评论必须出声。后端拒自赞走 `DomainError::Validation` ⇒ code **1002**，
 * 而按钮此前只认 1003（那是 web→api 网关不通）。结果：点自己的评论点赞，
 * 请求被拒但界面毫无反应（2026-10-21 实测 code/错误码表）。
 */

vi.mock("@/components/ui/toast", () => ({
  toast: { error: vi.fn(), success: vi.fn(), optimistic: vi.fn() },
}));

const SELF_LIKE = "参数校验失败: 不能给自己的评论点赞";

function res(body: unknown, status: number): Response {
  return {
    ok: status < 400,
    status,
    headers: { get: () => "application/json" },
    json: async () => body,
  } as unknown as Response;
}

beforeEach(() => {
  vi.clearAllMocks();
  document.cookie = "flux.session=1; path=/";
  document.cookie = "flux_csrf=abc123; path=/";
  vi.stubGlobal(
    "fetch",
    vi.fn(async () =>
      res(
        {
          code: 1002,
          message: SELF_LIKE,
          data: null,
          request_id: "req-self-like",
        },
        400,
      ),
    ),
  );
});

afterEach(() => {
  vi.unstubAllGlobals();
});

function mount() {
  return render(
    <LocaleProvider dict={zhCN} locale="zh-CN">
      <CommentLikeButton
        torrentId={7}
        commentId={99}
        initialLikes={0}
        initialLiked={false}
      />
    </LocaleProvider>,
  );
}

describe("评论点赞失败反馈", () => {
  it("自赞被拒（1002）时就地提示后端原句，不静默吞掉", async () => {
    mount();
    await userEvent.click(screen.getByRole("button"));
    await waitFor(() =>
      expect(vi.mocked(toast.error)).toHaveBeenCalledTimes(1),
    );
    expect(vi.mocked(toast.error).mock.calls[0][0]).toBe(SELF_LIKE);
    expect(vi.mocked(toast.error).mock.calls[0][1]).toEqual({
      requestId: "req-self-like",
    });
  });
});
