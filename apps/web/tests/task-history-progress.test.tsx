import { describe, it, expect } from "vitest";
import { render } from "@testing-library/react";

import { TaskHistory } from "@/components/task-board-parts";
import type { TaskOverview } from "@/components/task-board-parts";
import { LocaleProvider } from "@/i18n/client";
import { zhCN } from "@/i18n/zh-CN";

/**
 * 我的任务记录·进度列：每个指标键必须与 worker 结算同一把尺。
 * 现网踩过：seed_points_delta 的单位是「小时」（结算按 v×3600 秒判），
 * 旧渲染把它当秒再除 3600 ⇒ 45000 积分档位任务显示成 13h 目标。
 */

const GiB = 1024 ** 3;

const OV: TaskOverview = {
  shop: [],
  feed: [],
  stats: { ongoing: 3, done: 0, failed: 0, tiers: [] },
  my_records: [
    {
      task_id: 1,
      name: "做种研习社 · 做种积分 +1000",
      status: 0,
      claimed_at: "2026-10-01T00:00:00Z",
      settled_at: null,
      metric: { seed_points_delta: 1000 },
      current: { seed_seconds: 250 * 3600 },
    },
    {
      task_id: 4,
      name: "Master",
      status: 0,
      claimed_at: "2026-10-01T00:00:00Z",
      settled_at: null,
      metric: { download_delta: 10 * GiB },
      current: { downloaded: 5 * GiB },
    },
    {
      task_id: 9,
      name: "新人转正考核",
      status: 0,
      claimed_at: "2026-10-01T00:00:00Z",
      settled_at: null,
      metric: { seed_seconds_delta: 432000 },
      current: { seed_seconds: 60 * 3600 },
    },
  ],
};

function renderHistory() {
  return render(
    <LocaleProvider dict={zhCN} locale="zh-CN">
      <TaskHistory ov={OV} />
    </LocaleProvider>,
  );
}

describe("我的任务记录进度列", () => {
  it("seed_points_delta 目标按小时（点数）展示，不再除以 3600", () => {
    const { getByText, queryByText } = renderHistory();
    expect(
      getByText(`250.0 / ${(1000).toLocaleString()}h`),
    ).toBeInTheDocument();
    // 旧口径把 1000 积分当 1000 秒 ⇒ 目标折成 0h
    expect(queryByText("250.0 / 0h")).toBeNull();
  });

  it("download_delta 展示认领后下载增量现值", () => {
    const { getByText } = renderHistory();
    expect(getByText("5.00 GB / 10.00 GB")).toBeInTheDocument();
  });

  it("seed_seconds_delta 仍按秒折小时（回归钉）", () => {
    const { getByText } = renderHistory();
    expect(getByText("60.0 / 120h")).toBeInTheDocument();
  });
});
