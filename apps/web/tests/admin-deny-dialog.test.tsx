import { describe, it, expect, vi, beforeEach, afterEach } from "vitest";
import { render, screen, waitFor } from "@testing-library/react";
import userEvent from "@testing-library/user-event";

import { AdminDenyDialog } from "@/components/admin-deny-dialog";
import { LocaleProvider } from "@/i18n/client";
import { zhCN } from "@/i18n/zh-CN";

/**
 * 审核拒绝的理由弹窗（替换掉最后一处原生 `prompt()`）。
 *
 * 旧形态把 7 条理由拼成文本让版主「输入编号」，编号打错就静默把整句当自定义
 * 理由发出去；字典（torrent_deny_reasons）本来的意义就是结构化裁决。
 * 这里钉四件事：清单渲染成可选项、没选不许提交、选「其他」才逼出必填备注、
 * 载荷里 deny_reason_id / reason 二者不混发。
 * 外加一条兜底：字典读失败时不演成「本站没配理由」，而是明说没读到、
 * 仍可只写备注提交。
 */

const at = zhCN.adminTorrents;
const RS = [
  { id: 1, sort: 1, reason: "重复发布（伪原创）", enabled: true },
  { id: 3, sort: 3, reason: "资源质量不达标", enabled: true },
];

let replies: Record<string, unknown> = {};
let sent: Record<string, unknown> | null = null;
let done: string[] = [];

function env(data: unknown) {
  return { code: 0, message: "ok", data, request_id: "t" };
}

function res(body: unknown): Response {
  return {
    ok: true,
    status: 200,
    headers: { get: () => "application/json" },
    json: async () => body,
  } as unknown as Response;
}

function mount() {
  return render(
    <LocaleProvider dict={zhCN} locale="zh-CN">
      <AdminDenyDialog
        open
        torrentId={77}
        onClose={() => undefined}
        onDone={(m) => done.push(m)}
      />
    </LocaleProvider>,
  );
}

async function opened() {
  mount();
  const dlg = await screen.findByRole("dialog", { name: at.denyTitle });
  await waitFor(() =>
    expect(dlg.textContent).toContain(RS[0].reason),
  );
  return dlg;
}

beforeEach(() => {
  done = [];
  sent = null;
  replies = {
    reasons: env(RS),
    decide: env({ ok: true }),
  };
  document.cookie = "flux_csrf=abc123; path=/";
  vi.stubGlobal(
    "fetch",
    vi.fn(async (input: RequestInfo | URL, init?: RequestInit) => {
      const url = String(input);
      if (url.includes("/admin/deny-reasons")) return res(replies.reasons);
      if (url.includes("/admin/reviews/decide")) {
        sent = JSON.parse(String(init?.body ?? "{}"));
        return res(replies.decide);
      }
      return res(env(null));
    }),
  );
});

afterEach(() => {
  vi.unstubAllGlobals();
});

describe("审核拒绝理由弹窗", () => {
  it("字典渲染成可选项，没选之前不许提交", async () => {
    const dlg = await opened();
    expect(dlg.textContent).toContain(RS[1].reason);
    expect(dlg.textContent).toContain(at.denyOther);
    const submit = screen.getByRole("button", { name: at.denySubmit });
    expect((submit as HTMLButtonElement).disabled).toBe(true);
  });

  it("选字典项即放行，载荷带 deny_reason_id 不带自造 reason", async () => {
    await opened();
    await userEvent.click(screen.getByRole("radio", { name: RS[1].reason }));
    const submit = screen.getByRole("button", { name: at.denySubmit });
    expect((submit as HTMLButtonElement).disabled).toBe(false);
    await userEvent.click(submit);
    await waitFor(() =>
      expect(done).toEqual([at.rejectedMsg.replace("{id}", "77")]),
    );
    expect(sent).toEqual({
      torrent_id: 77,
      approve: false,
      reason: "",
      deny_reason_id: 3,
    });
  });

  it("选「其他」逼出必填备注，且不再发字典 id", async () => {
    const dlg = await opened();
    await userEvent.click(screen.getByRole("radio", { name: at.denyOther }));
    const submit = screen.getByRole("button", { name: at.denySubmit });
    expect((submit as HTMLButtonElement).disabled).toBe(true);
    expect(dlg.textContent).toContain(at.denyNoteShort);
    const note = dlg.querySelector("textarea") as HTMLTextAreaElement;
    await userEvent.type(note, " 封包内文件与标题不符 ");
    expect((submit as HTMLButtonElement).disabled).toBe(false);
    await userEvent.click(submit);
    expect(sent).toMatchObject({
      approve: false,
      reason: "封包内文件与标题不符",
    });
    expect((sent ?? {}).deny_reason_id).toBeUndefined();
  });

  it("字典读失败不演成「本站没配理由」，仍可只写备注提交", async () => {
    replies.reasons = { code: 1000, message: "上游炸了", data: null };
    mount();
    const dlg = await screen.findByRole("dialog", { name: at.denyTitle });
    await waitFor(() =>
      expect(dlg.textContent).toContain(at.denyLoadFail),
    );
    expect(dlg.textContent).not.toContain(at.denyEmptyDict);
    const note = dlg.querySelector("textarea") as HTMLTextAreaElement;
    await userEvent.type(note, "种子缺必要说明");
    await userEvent.click(screen.getByRole("button", { name: at.denySubmit }));
    expect(sent).toMatchObject({ reason: "种子缺必要说明", approve: false });
  });
});
