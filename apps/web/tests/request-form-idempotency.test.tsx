import { describe, it, expect, vi, beforeEach } from "vitest";
import { render, screen, fireEvent, waitFor } from "@testing-library/react";

import { RequestForm } from "@/components/request-board-form";
import { zhCN } from "@/i18n/zh-CN";

/**
 * P-632：求种发布的两条写口缺陷。
 * ① 后端对 title/descr 零上限：绕开界面可存任意大文本，列表会原样带给
 *    每个看榜人（后端闸门 R0b 取证）；这里只钉前端把两枚上限写进界面。
 * ② 悬赏冻结的幂等键由新建单 id 拼成 ⇒ 响应丢失后重试会再扣一次；
 *    表单须一次会话一枚键、失败复用、成功换新（同 buy-button）。
 */

const { post } = vi.hoisted(() => ({ post: vi.fn() }));

vi.mock("@/lib/api-client", async (importOriginal) => {
  const actual = await importOriginal<typeof import("@/lib/api-client")>();
  return { ...actual, api: { post } };
});

const t = zhCN.requests;

function mount() {
  return render(<RequestForm onCreated={vi.fn()} onCancel={vi.fn()} />);
}

function submit() {
  fireEvent.click(screen.getByRole("button", { name: t.formSubmit }));
}

function fillTitle(value: string) {
  fireEvent.change(screen.getByLabelText(t.formTitle), {
    target: { value },
  });
}

beforeEach(() => {
  post.mockReset();
});

describe("P-632 求种发布幂等", () => {
  it("失败重试复用同一枚键，不重新生成", async () => {
    post.mockRejectedValueOnce(new Error("网络中断"));
    post.mockResolvedValueOnce({});
    mount();
    fillTitle("求一个冷门种");
    submit();
    await screen.findByText("网络中断");
    expect(post.mock.calls[0][1].idempotency_key).toMatch(/^web-req-/);
    const first = post.mock.calls[0][1].idempotency_key;
    submit();
    await waitFor(() => expect(post).toHaveBeenCalledTimes(2));
    expect(post.mock.calls[1][1].idempotency_key).toBe(first);
  });

  it("成功后换新键，连续发布第二单不被幂等挡", async () => {
    post.mockResolvedValue({});
    mount();
    fillTitle("第一单");
    submit();
    await waitFor(() => expect(post).toHaveBeenCalledTimes(1));
    await screen.findByText(
      t.formOk.replace("{n}", "100").replaceAll("{magic}", "魔力"),
    );
    fillTitle("第二单");
    submit();
    await waitFor(() => expect(post).toHaveBeenCalledTimes(2));
    expect(post.mock.calls[1][1].idempotency_key).not.toBe(
      post.mock.calls[0][1].idempotency_key,
    );
  });

  it("两个长度上限仍以界面 maxLength 为准（200/2000）", () => {
    mount();
    expect(screen.getByLabelText(t.formTitle)).toHaveAttribute(
      "maxLength",
      "200",
    );
    expect(screen.getByLabelText(t.formDescr)).toHaveAttribute(
      "maxLength",
      "2000",
    );
  });
});
