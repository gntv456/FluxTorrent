import { describe, it, expect, vi, beforeEach, afterEach } from "vitest";
import { render, screen, waitFor } from "@testing-library/react";
import userEvent from "@testing-library/user-event";

import { TorrentDeleteDialog } from "@/components/torrent-delete-dialog";
import { LocaleProvider } from "@/i18n/client";
import { zhCN } from "@/i18n/zh-CN";

/**
 * 站方删除表单（NP delete.php 口径）的渲染层闸门。
 *
 * 站长 2026-10-16 报的形态是「删除弹窗缺字段」：旧界面只有一句
 * 「确认删除该种子？」，理由、说明、扣火花、封禁发种全都没有落点，
 * 裁决无从留痕。这一批把表单补齐后，本文件钉住四件只有挂真组件才
 * 看得见的东西：
 *  1) 图样要求的每个字段都渲染出来，且数字来自后端预览口（前端不自估）；
 *  2) 违规/其它 ⇒ 说明必填，没写就点不动（同时后端也拒，双层不脱节）；
 *  3) 提交的 JSON 载荷逐字段等于界面读数（勾选联动：双倍依附于扣除）；
 *  4) 删除已落库但处置失败时，界面先给失败清单，读完才放行 ——
 *     以及弹窗挂在 body 上（举报框刚犯过「被面板裁切」的同族错）。
 */

const T = zhCN.torrentManage2;
// P-558：`{magic}` 要换成站点货币名——旧断言拿模板原文当期望＝把缺陷钉成基线
const PREV = {
  owner_name: "uploader01",
  seeders: 0,
  leechers: 0,
  peers: 0,
  spark: 350,
  upload_banned: false,
  can_dispose: true,
};

let preview: Record<string, unknown> = { ...PREV };
let delReply: Record<string, unknown> = {};
let sent: string | null = null;
let closed = 0;
let gone = 0;

function env(data: unknown) {
  return { code: 0, message: "ok", data, request_id: "t" };
}

/** fetch 替身的最小响应体（只给 api-client 真正读的那几项） */
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
    <LocaleProvider dict={zhCN} locale="zh-CN" currency="魔力">
      <TorrentDeleteDialog
        open
        torrentId={42}
        onClose={() => {
          closed += 1;
        }}
        onDeleted={() => {
          gone += 1;
        }}
      />
    </LocaleProvider>,
  );
}

beforeEach(() => {
  closed = 0;
  gone = 0;
  sent = null;
  preview = { ...PREV };
  delReply = env({
    deleted: 42,
    sanctions: {
      spark_base: 350,
      spark_deducted: 700,
      spark_replayed: false,
      upload_banned: true,
      errors: [],
    },
  });
  // api-client 的 CSRF 双提交：给一个已存在的 cookie，免掉续期请求
  document.cookie = "flux_csrf=abc123; path=/";
  vi.stubGlobal(
    "fetch",
    vi.fn(async (input: RequestInfo | URL, init?: RequestInit) => {
      const url = String(input);
      if (init?.method === "DELETE") sent = String(init.body ?? "");
      const body = url.includes("delete-preview") ? env(preview) : delReply;
      return res(body);
    }),
  );
});

afterEach(() => {
  vi.unstubAllGlobals();
});

/** 打开弹窗并等预览口回数据（同伴那行只有拿到 pv 才会渲染） */
async function opened() {
  mount();
  const dlg = await screen.findByRole("dialog", { name: T.delTitle });
  await waitFor(() =>
    expect(dlg.textContent).toContain(
      T.delPeers
        .replace("{seed}", String(PREV.seeders))
        .replace("{leech}", String(PREV.leechers))
        .replace("{peers}", String(PREV.peers)),
    ),
  );
  return dlg;
}

describe("站方删除表单（渲染层）", () => {
  it("图样要求的字段一个都不缺，数字取自后端预览口", async () => {
    const dlg = await opened();
    const txt = dlg.textContent ?? "";
    // P-558：不许露 {magic}，也不许被 fmt 折成空串少一截
    expect(txt).not.toContain("{magic}");
    expect(txt).toContain("获得的魔力：350点");
    for (const want of [
      T.delReason,
      T.delNote,
      T.delReasonDead,
      T.delReasonDuplicate,
      T.delReasonLowQuality,
      T.delReasonViolation,
      T.delReasonOther,
      T.delSpark.replaceAll("{magic}", "魔力"),
      T.delDouble.replaceAll("{magic}", "魔力"),
      T.delBan,
      T.delBanNote,
    ]) {
      expect(txt).toContain(want);
    }
    // 「0 做种者 + 0 下载者 = 0 总同伴」：三个数都来自预览口
    expect(txt).toContain(
      T.delPeers
        .replace("{seed}", String(PREV.seeders))
        .replace("{leech}", String(PREV.leechers))
        .replace("{peers}", String(PREV.peers)),
    );
    expect(txt).toContain("700");
    // 预览口确实被打过
    expect(
      [...(globalThis.fetch as ReturnType<typeof vi.fn>).mock.calls].some((c) =>
        String(c[0]).endsWith("/api/v1/torrents/42/delete-preview"),
      ),
    ).toBe(true);
  });

  it("弹窗挂在 body 上（不是面板内的绝对定位小块）", async () => {
    const dlg = await opened();
    const scrim = dlg.parentElement as HTMLElement;
    expect(scrim.parentElement).toBe(document.body);
    expect(scrim.className).toContain("fixed");
  });

  it("违规/其它：不写说明就点不动，写了才解禁", async () => {
    const dlg = await opened();
    const ok = screen.getByRole("button", { name: T.delOk });
    const note = dlg.querySelector("textarea") as HTMLTextAreaElement;
    const sel = dlg.querySelector("select") as HTMLSelectElement;
    // 默认理由「断种」不要求说明
    expect(sel.value).toBe("dead");
    expect((ok as HTMLButtonElement).disabled).toBe(false);
    await userEvent.selectOptions(sel, "violation");
    expect((ok as HTMLButtonElement).disabled).toBe(true);
    await userEvent.clear(note);
    await userEvent.type(note, "   ");
    expect((ok as HTMLButtonElement).disabled).toBe(true);
    await userEvent.clear(note);
    await userEvent.type(note, "违规资源");
    expect((ok as HTMLButtonElement).disabled).toBe(false);
  });

  it("双倍依附于扣除；无可回收火花时两项都锁死", async () => {
    const dlg = await opened();
    const [spark, dbl, ban] = [
      ...dlg.querySelectorAll<HTMLInputElement>('input[type="checkbox"]'),
    ];
    expect(spark.disabled).toBe(false);
    expect(dbl.disabled).toBe(true);
    await userEvent.click(spark);
    expect(dbl.disabled).toBe(false);
    await userEvent.click(ban);
    expect(ban.checked).toBe(true);
  });

  it("发种人从这颗种没入袋过 ⇒ 扣除与双倍都锁死", async () => {
    preview = { ...PREV, spark: 0 };
    const dlg = await opened();
    const boxes = [
      ...dlg.querySelectorAll<HTMLInputElement>('input[type="checkbox"]'),
    ];
    expect(boxes[0].disabled).toBe(true);
    expect(boxes[1].disabled).toBe(true);
  });

  it("can_dispose=false 时整块处置换成一句说明，不作假收口", async () => {
    preview = { ...PREV, can_dispose: false };
    const dlg = await opened();
    const txt = dlg.textContent ?? "";
    expect(txt).toContain(T.delNoDispose);
    expect(txt).not.toContain(T.delSpark);
  });

  it("提交的载荷逐字段等于界面读数", async () => {
    const dlg = await opened();
    const sel = dlg.querySelector("select") as HTMLSelectElement;
    const note = dlg.querySelector("textarea") as HTMLTextAreaElement;
    const [spark, dbl, ban] = [
      ...dlg.querySelectorAll<HTMLInputElement>('input[type="checkbox"]'),
    ];
    await userEvent.selectOptions(sel, "duplicate");
    await userEvent.type(note, "  同种重复  ");
    await userEvent.click(spark);
    await userEvent.click(dbl);
    await userEvent.click(ban);
    await userEvent.click(screen.getByRole("button", { name: T.delOk }));
    await waitFor(() => expect(gone).toBe(1));
    expect(JSON.parse(sent ?? "{}")).toEqual({
      reason: "duplicate",
      note: "同种重复",
      deduct_spark: true,
      double_spark: true,
      ban_upload: true,
    });
    // 成功且无失败项：不该出现失败清单
    expect(closed).toBe(0);
  });

  it("删除已落库但处置失败 ⇒ 先读清单，点「知道了」才离开", async () => {
    delReply = env({
      deleted: 42,
      sanctions: {
        spark_base: 350,
        spark_deducted: 0,
        spark_replayed: false,
        upload_banned: false,
        errors: ["火花回收失败：余额不足", "封禁发种权限失败：写库错误"],
      },
    });
    await opened();
    await userEvent.click(screen.getByRole("button", { name: T.delOk }));
    await screen.findByText(T.delPartial);
    const body = document.body.textContent ?? "";
    expect(body).toContain("火花回收失败：余额不足");
    expect(body).toContain("封禁发种权限失败：写库错误");
    // 还没读完不许走
    expect(gone).toBe(0);
    await userEvent.click(screen.getByRole("button", { name: T.delGotIt }));
    await waitFor(() => expect(gone).toBe(1));
  });

  it("后端拒单（400）时弹窗不关、原因照抄给用户", async () => {
    delReply = {
      code: 1002,
      message: "参数校验失败: 违规与其它理由必须填写说明",
      data: null,
    };
    const dlg = await opened();
    const sel = dlg.querySelector("select") as HTMLSelectElement;
    const note = dlg.querySelector("textarea") as HTMLTextAreaElement;
    // 前端必填联动放行后，仍会被后端那一层拒（这里演的就是这一层）
    await userEvent.selectOptions(sel, "dead");
    await userEvent.type(note, "断种");
    await userEvent.click(screen.getByRole("button", { name: T.delOk }));
    await waitFor(() =>
      expect(dlg.textContent).toContain("违规与其它理由必须填写说明"),
    );
    expect(gone).toBe(0);
  });

  it("预览口挂掉时不拦住删除，但必须出声", async () => {
    vi.stubGlobal(
      "fetch",
      vi.fn(async (input: RequestInfo | URL, init?: RequestInit) => {
        const url = String(input);
        if (url.includes("delete-preview")) throw new Error("network down");
        if (init?.method === "DELETE") sent = String(init.body ?? "");
        return res(delReply);
      }),
    );
    mount();
    const dlg = await screen.findByRole("dialog", { name: T.delTitle });
    await waitFor(() =>
      expect(dlg.textContent).toContain(T.delLoadFail),
    );
    expect(dlg.textContent).not.toContain(T.delSpark);
    await userEvent.click(screen.getByRole("button", { name: T.delOk }));
    await waitFor(() => expect(gone).toBe(1));
    expect(JSON.parse(sent ?? "{}").deduct_spark).toBe(false);
  });
});
