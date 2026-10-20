import { describe, it, expect, vi, beforeEach, afterEach } from "vitest";
import { render, screen, waitFor } from "@testing-library/react";
import userEvent from "@testing-library/user-event";

import { api, ApiError } from "@/lib/api-client";
import { DownloadButton } from "@/components/download-button";
import { toast } from "@/components/ui/toast";
import { LocaleProvider } from "@/i18n/client";
import { zhCN } from "@/i18n/zh-CN";

/**
 * 下载失败必须说清「为什么、该怎么办」（批二百零三）。
 *
 * 现网实测（2026-10-20，root 会话）：`GET /api/v1/torrents/49417/download`
 * 回 `400 {"code":1002,"message":"参数校验失败: 种子文件缺失（原始 .torrent
 * 未存档），请联系管理组补档","request_id":…}`。后端刻意区分「种不存在」与
 * 「种在但原文丢失」（download.rs 评审 P1-4），下载资格暂停
 * （charge.rs:44）、火花不足（4001/409）同理——可行动的中文全在信封里。
 *
 * 而 blob 口 `requestBlob` 原先只看 HTTP 状态码，造一句
 * 「下载失败（HTTP 400）」就把这句丢了：被停下载/余额不足/管理组没补档
 * 三种完全不同的处境，界面上长得一模一样，会员只能盲重试或报障。
 * 四条 getBlob 出口（详情页下载钮、列表行、移动吸底条、字幕下载）都吃这个洞。
 *
 * 负控：把 requestBlob 换回旧实现（只按 res.status 造 ApiError）后，
 * 本文件第 1、3、4 条必须全红；把两个出口的文案改回 `e.message`
 * 直抛，第 4 条必须红（标签前缀「参数校验失败: 」会摊到界面）。
 */

/** blob 失败信封：status/ctype/body 逐一对齐后端真实响应 */
function blobFail(
  status: number,
  ctype: string,
  body: unknown,
): ReturnType<typeof vi.fn> {
  const fn = vi.fn(async () =>
    ({
      ok: status >= 200 && status < 300,
      status,
      headers: new Headers({ "content-type": ctype }),
      json: async () => body,
      arrayBuffer: async () => new ArrayBuffer(0),
    }) as unknown as Response,
  );
  vi.stubGlobal("fetch", fn);
  return fn;
}

const MISSING_ARCHIVE = {
  code: 1002,
  message: "参数校验失败: 种子文件缺失（原始 .torrent 未存档），请联系管理组补档",
  data: null,
  request_id: "c1967f92-ccfe-4474-a468-9cd007cf1d68",
};

let errSpy: ReturnType<typeof vi.spyOn>;

beforeEach(() => {
  errSpy = vi.spyOn(toast, "error").mockImplementation(() => 0);
});

afterEach(() => {
  vi.restoreAllMocks();
  vi.unstubAllGlobals();
});

describe("getBlob 失败归一（api-client）", () => {
  it("信封里的业务码与中文判据必须原样交给调用方", async () => {
    blobFail(400, "application/json", MISSING_ARCHIVE);
    const e = await api
      .getBlob("/api/v1/torrents/49417/download")
      .catch((x) => x);
    expect(e).toBeInstanceOf(ApiError);
    expect(e.code).toBe(1002);
    expect(e.message).toBe(MISSING_ARCHIVE.message);
    expect(e.status).toBe(400);
    expect(e.requestId).toBe(MISSING_ARCHIVE.request_id);
  });

  it("网关 HTML（非信封）仍回落兜底文案，不演成读不出来的错", async () => {
    blobFail(502, "text/html", null);
    const e = await api
      .getBlob("/api/v1/torrents/1/download")
      .catch((x) => x);
    expect(e).toBeInstanceOf(ApiError);
    expect(e.code).toBe(1000);
    expect(e.message).toContain("502");
  });

  it("401 仍按「请先登录」归一（口径不变）", async () => {
    blobFail(401, "application/json", {
      code: 2001,
      message: "未认证",
      data: null,
      request_id: "x",
    });
    const e = await api.getBlob("/api/v1/torrents/1/download").catch((x) => x);
    expect(e.code).toBe(2001);
    expect(e.message).not.toBe("");
  });
});

describe("下载出口必须把可行动原因送到界面", () => {
  function mount(node: React.ReactNode) {
    return render(
      <LocaleProvider dict={zhCN} locale="zh-CN">
        {node}
      </LocaleProvider>,
    );
  }

  it("详情页下载钮：toast 报的是后端那句中文（去掉冗余标签）", async () => {
    blobFail(400, "application/json", MISSING_ARCHIVE);
    mount(
      <DownloadButton
        torrentId={49417}
        name="什么意思夫妇"
        showMagnet={false}
      />,
    );
    await userEvent.click(
      await screen.findByRole("button", { name: zhCN.torrent.download }),
    );
    await waitFor(() => expect(errSpy).toHaveBeenCalled());
    // apiErrorMessage（P-305）：以本码字典标签开头 ⇒ 只展示冒号后的正文
    expect(errSpy.mock.calls[0][0]).toBe(
      "种子文件缺失（原始 .torrent 未存档），请联系管理组补档",
    );
  });

  it("下载资格被暂停：界面说的是「联系管理组」而不是 HTTP 400", async () => {
    blobFail(400, "application/json", {
      code: 1002,
      message: "参数校验失败: 您的下载权限已被暂停，请联系管理组",
      data: null,
      request_id: "r-2",
    });
    mount(
      <DownloadButton
        torrentId={49417}
        name="沙丘：复仇"
        showMagnet={false}
      />,
    );
    await userEvent.click(
      await screen.findByRole("button", { name: zhCN.torrent.download }),
    );
    await waitFor(() => expect(errSpy).toHaveBeenCalled());
    expect(errSpy.mock.calls[0][0]).toBe(
      "您的下载权限已被暂停，请联系管理组",
    );
  });
});
