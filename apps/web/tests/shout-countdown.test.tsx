import { describe, it, expect, vi, beforeEach, afterEach } from "vitest";
import { render, screen, act } from "@testing-library/react";

// mock api：记录 /shoutbox 拉取次数，返回一条消息
let shoutboxCalls = 0;
vi.mock("@/lib/api-client", async (importOriginal) => {
  const orig = await importOriginal<typeof import("@/lib/api-client")>();
  return {
    ...orig,
    api: {
      get: vi.fn(async (path: string) => {
        if (path === "/api/v1/shoutbox") {
          shoutboxCalls += 1;
          return [{ id: 1, username: "root", message: "hi", created_at: "2026-11-09T00:00:00Z" }];
        }
        if (path === "/api/v1/shoutbox/bot") return { commands: [] };
        if (path === "/api/v1/me") return { class_id: 99 };
        return null;
      }),
      post: vi.fn(async () => ({})),
    },
  };
});

import { ShoutBox } from "../components/shout-box";

describe("ShoutBox 可见刷新倒计时（方案 B）", () => {
  beforeEach(() => {
    shoutboxCalls = 0;
    vi.useFakeTimers();
  });
  afterEach(() => {
    vi.useRealTimers();
  });

  it("标题显示倒计时并逐秒递减，到 0 触发一次重新拉取后重置", async () => {
    render(<ShoutBox active />);
    // 首屏 load（异步）先跑一拍
    await act(async () => { await Promise.resolve(); });
    expect(shoutboxCalls).toBe(1);
    // 初始倒计时 15
    expect(screen.getByText(/15 秒后刷新/)).toBeInTheDocument();

    // 推进 1 秒 → 14
    await act(async () => { await vi.advanceTimersByTimeAsync(1000); });
    expect(screen.getByText(/14 秒后刷新/)).toBeInTheDocument();

    // 再推进 3 秒 → 11
    await act(async () => { await vi.advanceTimersByTimeAsync(3000); });
    expect(screen.getByText(/11 秒后刷新/)).toBeInTheDocument();

    // 推进剩余 11 秒到 0：应触发一次 load 并重置回 15
    await act(async () => { await vi.advanceTimersByTimeAsync(11000); });
    expect(shoutboxCalls).toBe(2); // 首屏 + 归零一次
    expect(screen.getByText(/15 秒后刷新/)).toBeInTheDocument();
  });

  it("active=false（手机端桌面隐去那份）不倒计时也不轮询", async () => {
    render(<ShoutBox active={false} />);
    await act(async () => { await vi.advanceTimersByTimeAsync(20000); });
    expect(shoutboxCalls).toBe(0);
  });
});
