import { describe, it, expect, vi, beforeEach, afterEach } from "vitest";
import { render, screen, waitFor } from "@testing-library/react";
import userEvent from "@testing-library/user-event";

import { PasskeyRotate } from "@/components/passkey-rotate";
import { LocaleProvider } from "@/i18n/client";
import { zhCN } from "@/i18n/zh-CN";

/**
 * P-409：轮换下载密钥必须先复核本人。
 *
 * 后端已给 /me/passkey/rotate 接上 step_up（密码必对；开了 2FA 再要动态码）
 * 与 5 次/分限流。界面这一侧要证明：① 没填密码时按钮不可点；② 请求体真的
 * 带上 password / code；③ 被拒时把后端那句中文显示出来，不假装成功。
 */

const S = zhCN.usercp.security;
const M = zhCN.my;
const CONFIRM = zhCN.ui.confirmDelete;
let body: Record<string, unknown> | null = null;
let fail = false;

function res(payload: unknown, status = 200): Response {
  return {
    ok: status >= 200 && status < 300,
    status,
    headers: { get: () => "application/json" },
    json: async () => payload,
  } as unknown as Response;
}

function stub() {
  vi.stubGlobal(
    "fetch",
    vi.fn(async (input: RequestInfo | URL, init?: RequestInit) => {
      const url = String(input);
      if (
        init?.method === "POST" &&
        url.endsWith("/me/passkey/rotate")
      ) {
        body = JSON.parse(String(init.body ?? "{}"));
        if (fail) {
          return res(
            {
              code: 1002,
              message: "参数校验失败: 重置密钥 未执行：凭证无效",
              data: null,
              request_id: "t",
            },
            400,
          );
        }
        return res({
          code: 0,
          data: { passkey: "newpk123" },
          request_id: "t",
        });
      }
      return res({ code: 0, data: null, request_id: "t" });
    }),
  );
}

beforeEach(() => {
  body = null;
  fail = false;
  document.cookie = "flux.session=1; path=/";
  document.cookie = "flux_csrf=abc123; path=/";
});

afterEach(() => {
  vi.unstubAllGlobals();
});

function mount() {
  return render(
    <LocaleProvider dict={zhCN} locale="zh-CN">
      <PasskeyRotate />
    </LocaleProvider>,
  );
}

async function fillAndConfirm(pw: string, code?: string) {
  await userEvent.type(screen.getByPlaceholderText(S.oldPassword), pw);
  if (code) {
    await userEvent.type(
      screen.getByPlaceholderText(zhCN.security2fa.codeLabel),
      code,
    );
  }
  await userEvent.click(
    screen.getByRole("button", { name: M.passkeyRotate }),
  );
  await userEvent.click(
    await screen.findByRole("button", { name: CONFIRM }),
  );
}

describe("轮换下载密钥的再验证（P-409）", () => {
  it("密码为空时按钮不可点；填密码后请求体带 password", async () => {
    stub();
    mount();
    const btn = screen.getByRole("button", { name: M.passkeyRotate });
    expect((btn as HTMLButtonElement).disabled).toBe(true);
    await fillAndConfirm("s3cret-pass");
    await waitFor(() => expect(body).not.toBeNull());
    expect(body).toEqual({ password: "s3cret-pass" });
    expect(await screen.findByText(/newpk123/)).toBeTruthy();
    expect(screen.getByText(M.passkeyRotated)).toBeTruthy();
  });

  it("填了动态码就按整数一并提交", async () => {
    stub();
    mount();
    await fillAndConfirm("s3cret-pass", "012345");
    await waitFor(() => expect(body).not.toBeNull());
    expect(body).toEqual({ password: "s3cret-pass", code: 12345 });
  });

  it("复核被拒时显示后端中文，不显示新密钥", async () => {
    stub();
    fail = true;
    mount();
    await fillAndConfirm("wrong-pass");
    expect(
      await screen.findByText("重置密钥 未执行：凭证无效"),
    ).toBeTruthy();
    expect(screen.queryByText(/newpk123/)).toBeNull();
    expect(screen.queryByText(M.passkeyRotated)).toBeNull();
  });
});
