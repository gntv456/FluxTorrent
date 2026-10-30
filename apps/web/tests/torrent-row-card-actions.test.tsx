import { describe, expect, it, vi } from "vitest";
import { fireEvent, render, screen, waitFor } from "@testing-library/react";

vi.mock("@/lib/api-client", () => ({
  api: {
    getBlob: vi.fn(async () => new ArrayBuffer(1)),
    put: vi.fn(async () => ({ bookmarked: true })),
  },
  ApiError: class ApiError extends Error {},
}));

import { TorrentRowCard } from "@/components/torrent-row-card";
import { zhCN } from "@/i18n/zh-CN";
import { api } from "@/lib/api-client";

/** 移动端行卡片滑动操作：下载走 getBlob、收藏走 bookmark PUT，
 *  旧实现把下载指到未消费的 ?dl=1、收藏只收起行。 */
describe("移动端行卡片快捷操作", () => {
  it("下载与收藏都发出真实请求", async () => {
    Object.defineProperty(URL, "createObjectURL", {
      value: vi.fn(() => "blob:x"),
      configurable: true,
    });
    Object.defineProperty(URL, "revokeObjectURL", {
      value: vi.fn(),
      configurable: true,
    });
    vi.spyOn(HTMLAnchorElement.prototype, "click").mockImplementation(
      () => undefined,
    );
    render(
      <TorrentRowCard
        t={{
          id: 5,
          name: "Movie 2024",
          size: 1024,
          seeders: 1,
          leechers: 2,
          times_completed: 0,
          category_id: 1,
          sec_names: [],
        }}
      />,
    );
    fireEvent.click(
      screen.getByRole("button", { name: zhCN.torrents.download }),
    );
    await waitFor(() =>
      expect(api.getBlob).toHaveBeenCalledWith(
        "/api/v1/torrents/5/download",
      ),
    );
    fireEvent.click(
      screen.getByRole("button", { name: zhCN.torrents.bookmark }),
    );
    await waitFor(() =>
      expect(api.put).toHaveBeenCalledWith(
        "/api/v1/torrents/5/bookmark",
        { on: true },
      ),
    );
  });
});
