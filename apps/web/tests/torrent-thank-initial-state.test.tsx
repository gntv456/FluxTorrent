import { describe, it, expect, vi, beforeEach, afterEach } from "vitest";
import { render, screen, waitFor } from "@testing-library/react";
import userEvent from "@testing-library/user-event";

import { api } from "@/lib/api-client";
import {
  ThankButton,
  TorrentSocial,
} from "@/components/torrent-social";
import { LocaleProvider } from "@/i18n/client";
import { zhCN } from "@/i18n/zh-CN";

/**
 * 感谢按钮的初始态与同页同步（2026-10-20）。
 *
 * 后端 thanks 是 (torrent_id,user_id) 主键：一人一种只能感谢一次，
 * 再次提交（含带金额的答谢）一律回 code 5002。可详情聚合原先不带
 * 「本人已感谢」位，按钮 useState(false) 写死——刷新后仍显示「感谢
 * 发布者」，点下去必失败；头部与面板两枚按钮也不互通。
 *
 * 负控：把两处 useState(initialThanked) 改回 useState(false)，
 * 初始态两条必须变红；去掉 announceThanked，同步那条必须变红。
 */

function mount(node: React.ReactNode) {
  return render(
    <LocaleProvider dict={zhCN} locale="zh-CN">
      {node}
    </LocaleProvider>,
  );
}

beforeEach(() => {
  document.cookie = "flux.session=1; path=/";
});

afterEach(() => {
  vi.restoreAllMocks();
  document.cookie = "flux.session=; path=/; max-age=0";
});

describe("感谢按钮初始态", () => {
  it("已感谢用户刷新后头部按钮显示「已感谢」且不可再点", () => {
    mount(<ThankButton torrentId={50053} initialThanked />);
    const btn = screen.getByRole("button", {
      name: zhCN.torrent.thanked,
    });
    expect(btn).toBeDisabled();
  });

  it("未感谢用户仍显示「感谢发布者」（正控）", () => {
    mount(<ThankButton torrentId={50053} />);
    const btn = screen.getByRole("button", {
      name: zhCN.torrent.thanks,
    });
    expect(btn).toBeEnabled();
  });

  it("已感谢用户面板内魔力答谢组整体禁用", async () => {
    mount(<TorrentSocial torrentId={50053} initialThanked />);
    const tip = await screen.findByRole("button", { name: "+1" });
    expect(tip).toBeDisabled();
    expect(screen.getByText(zhCN.torrent.thanked)).toBeTruthy();
  });

  it("未感谢用户答谢组可用（正控）", async () => {
    mount(<TorrentSocial torrentId={50053} />);
    const tip = await screen.findByRole("button", { name: "+1" });
    expect(tip).toBeEnabled();
  });

  it("头部致谢成功后同页面板内按钮同步为已感谢", async () => {
    vi.spyOn(api, "post").mockResolvedValue({
      spark_given: 0,
    } as never);
    mount(
      <>
        <ThankButton torrentId={50053} />
        <ThankButton torrentId={50053} />
      </>,
    );
    const before = screen.getAllByRole("button", {
      name: zhCN.torrent.thanks,
    });
    expect(before).toHaveLength(2);
    await userEvent.click(before[0]);
    await waitFor(() => {
      const after = screen.getAllByRole("button", {
        name: zhCN.torrent.thanked,
      });
      expect(after).toHaveLength(2);
      expect(after.every((b) => (b as HTMLButtonElement).disabled)).toBe(
        true,
      );
    });
  });
});
