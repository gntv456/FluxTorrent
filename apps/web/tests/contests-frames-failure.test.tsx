import { describe, it, expect, vi, afterEach } from "vitest";
import { render, screen } from "@testing-library/react";

import { ContestBoard } from "@/components/plugins";
import { FrameShop } from "@/components/plugins-frames";
import { LocaleProvider } from "@/i18n/client";
import { zhCN } from "@/i18n/zh-CN";

/** 插件前台两处：大赛列表与挂件商店取数失败不能演成
 *  「暂无大赛」「本站没有挂件」。 */

afterEach(() => vi.unstubAllGlobals());

function envelope(data: unknown) {
  return {
    ok: true,
    status: 200,
    headers: { get: () => "application/json" },
    json: async () => ({ code: 0, message: "ok", data, request_id: "t" }),
  } as unknown as Response;
}

function mount(node: React.ReactNode) {
  return render(
    <LocaleProvider dict={zhCN} locale="zh-CN">{node}</LocaleProvider>,
  );
}

describe("大赛列表失败不演空", () => {
  it("取数失败：报加载失败，不报「暂无大赛」", async () => {
    vi.stubGlobal(
      "fetch",
      vi.fn(async () => {
        throw new Error("network down");
      }),
    );
    mount(<ContestBoard />);
    expect(await screen.findByText(zhCN.common.loadFailed)).toBeTruthy();
    expect(screen.queryByText(zhCN.contests2.empty)).toBeNull();
  });

  it("真实空列表：仍报「暂无大赛」", async () => {
    vi.stubGlobal(
      "fetch",
      vi.fn(async () => envelope([])),
    );
    mount(<ContestBoard />);
    expect(await screen.findByText(zhCN.contests2.empty)).toBeTruthy();
    expect(screen.queryByText(zhCN.common.loadFailed)).toBeNull();
  });
});

describe("挂件商店失败不演空", () => {
  it("取数失败：报加载失败", async () => {
    vi.stubGlobal(
      "fetch",
      vi.fn(async () => {
        throw new Error("network down");
      }),
    );
    mount(<FrameShop current={null} />);
    expect(await screen.findByText(zhCN.common.loadFailed)).toBeTruthy();
  });

  it("成功：渲染挂件卡片", async () => {
    vi.stubGlobal(
      "fetch",
      vi.fn(async () =>
        envelope([{ id: 7, name: "星屑挂件", css: "", price: 0 }]),
      ),
    );
    mount(<FrameShop current={null} />);
    expect(await screen.findByText("星屑挂件")).toBeTruthy();
    expect(screen.queryByText(zhCN.common.loadFailed)).toBeNull();
  });
});
