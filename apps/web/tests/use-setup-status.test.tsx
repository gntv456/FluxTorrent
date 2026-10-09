import { describe, it, expect, vi, beforeEach } from "vitest";
import {
  render,
  screen,
  waitFor,
  fireEvent,
  act,
} from "@testing-library/react";
import { useSetupStatus } from "@/app/setup/_parts/use-setup-status";

/**
 * 安装向导首屏加载的回归闸门。
 *
 * 真缺陷：api 进程先跑完全部 sqlx 迁移才 bind 端口（main.rs 迁移在
 * HttpServer::bind 之前），全新装机时 web 已 healthy、/setup 能渲染，
 * 但 /api/v1/setup/status 必然连不上。旧实现 catch 里写死一句话且
 * 不重试 —— 站长在冷启动窗口打开向导就永久卡死，只能手动刷页面；
 * 同时 ApiError 的业务码被丢弃，1003（web→api 网关不通）与浏览器
 * 网络故障渲染成同一句话，诊断线索销毁。
 *
 * 此用例锁住三件事：自动重试能恢复、真实原因（业务码）透出、
 * 达到上限后停止自动探测但保留手动重试入口。
 */

const t = (_k: string, fallback: string) => fallback;

function Probe() {
  const boot = useSetupStatus(t);
  if (boot.error) {
    return (
      <div>
        <p data-testid="err">{boot.error}</p>
        <p data-testid="detail">{boot.detail}</p>
        <p data-testid="hint">{boot.hint}</p>
        <button onClick={boot.reload} disabled={boot.probing}>
          reload
        </button>
        <span data-testid="attempt">{boot.attempt}</span>
      </div>
    );
  }
  return <p data-testid="ok">{boot.status ? "loaded" : "loading"}</p>;
}

beforeEach(() => {
  vi.restoreAllMocks();
  vi.useRealTimers();
});

describe("useSetupStatus（向导首屏加载）", () => {
  it("首屏失败后自动重试，恢复即渲染内容", async () => {
    const { api, ApiError } = await import("@/lib/api-client");
    let calls = 0;
    vi.spyOn(api, "get").mockImplementation(() => {
      calls += 1;
      // 前两次失败（冷启动窗口），第三次成功
      if (calls < 3) return Promise.reject(new ApiError(1003, "网关无响应"));
      return Promise.resolve({
        done: false,
        has_admin: false,
        packs: [],
      }) as never;
    });

    render(<Probe />);
    // 首屏：失败原因必须带出业务码，不能被压成一句话
    await waitFor(() => expect(screen.getByTestId("detail")).toBeTruthy());
    expect(screen.getByTestId("detail").textContent).toContain("1003");

    // 退避重试会自动发生，最终拿到状态
    await waitFor(
      () => expect(screen.getByTestId("ok").textContent).toBe("loaded"),
      { timeout: 5000 },
    );
    expect(calls).toBeGreaterThanOrEqual(3);
  });

  it("非 ApiError（浏览器网络故障）不伪造业务码，按网络建议分类", async () => {
    const { api } = await import("@/lib/api-client");
    vi.spyOn(api, "get").mockImplementation(() =>
      Promise.reject(new TypeError("Failed to fetch")),
    );
    render(<Probe />);
    await waitFor(() => expect(screen.getByTestId("detail")).toBeTruthy());
    const detail = screen.getByTestId("detail").textContent ?? "";
    expect(detail).toContain("浏览器请求失败");
    expect(detail).not.toContain("code 1003");
  });

  it("持续失败到上限后停止自动探测，手动重试仍可用", async () => {
    vi.useFakeTimers();
    const { api, ApiError } = await import("@/lib/api-client");
    const get = vi
      .spyOn(api, "get")
      .mockImplementation(() =>
        Promise.reject(new ApiError(1003, "网关无响应")),
      );

    render(<Probe />);
    await vi.waitFor(() =>
      expect(screen.getByTestId("attempt").textContent).toBe("1"),
    );

    // 推进所有退避定时器
    for (let i = 0; i < 15; i += 1) {
      await vi.advanceTimersByTimeAsync(30_000);
    }
    // 首次 + 11 次重试 = 12 次（MAX_ATTEMPTS），之后不再自动探测
    expect(get).toHaveBeenCalledTimes(12);

    const before = get.mock.calls.length;
    await vi.advanceTimersByTimeAsync(60_000);
    expect(get).toHaveBeenCalledTimes(before);

    // 手动重试依然能重新发起（fireEvent 而非 userEvent：后者在
    // 假定时器下会挂住 —— 它内部有依赖真实时器的 advance 循环）
    await act(async () => {
      fireEvent.click(screen.getByText("reload"));
    });
    await vi.waitFor(() => expect(get.mock.calls.length).toBe(before + 1));
  }, 20_000);
});
