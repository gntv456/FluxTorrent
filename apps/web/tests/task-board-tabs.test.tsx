import { describe, it, expect, vi, afterEach, beforeEach } from "vitest";
import { render, screen, waitFor, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";

import { TaskBoard } from "@/components/task-board";
import { LocaleProvider } from "@/i18n/client";
import { zhCN } from "@/i18n/zh-CN";

/**
 * 任务页收进两组标签（对齐样图 § tplTask）。
 *
 * 钉住四件事：
 *  1. 内容真的被收纳了——不在当前标签的面板不得留在 DOM 里
 *     （否则「不平铺」只是把重复标题删掉，页面照样一万像素高）；
 *  2. 两组标签互不干扰——切「我的任务记录」不许把「任务商店」一起切走；
 *  3. 标题层级唯一——面板内不再自带与标签同名的第二级 h2；
 *  4. 新原语的键盘可达（←/→ 换标签）与 aria-controls ↔ labelledby 双向指认。
 */

const t = zhCN.tasks2;
const GiB = 1024 ** 3;

const TASKS = [
  {
    id: 11,
    name: "骨灰",
    metric: {
      upload_delta: 20 * GiB,
      download_delta: 10 * GiB,
      seed_points_delta: 2000,
    },
    reward: 5000,
    penalty: 2000,
    claim_limit: null,
    claimed: 12,
    starts_at: "2026-10-01T00:00:00Z",
    ends_at: "2026-10-31T00:00:00Z",
    tier: "master",
    subtitle: "20GB 上传挑战 · 大师级",
    fee: 500,
    duration_days: 30,
    quota_total: 200,
    claimed_by_me: false,
  },
  {
    id: 12,
    name: "字幕贡献",
    metric: { subtitles: 12 },
    reward: 1200,
    penalty: 400,
    claim_limit: 20,
    claimed: 8,
    starts_at: "2026-10-01T00:00:00Z",
    ends_at: "2026-10-31T00:00:00Z",
    claimed_by_me: true,
  },
];

const OV = {
  shop: [
    {
      name: "上传加速卡",
      span: "月度权益",
      require_tier: "master",
      require_count: 2,
      cost: 8000,
      stock: 50,
    },
  ],
  feed: [{ user: "青柠", task: "烧糊涂", at: "2026-10-16T21:34:00Z" }],
  stats: {
    ongoing: 1,
    done: 2,
    failed: 0,
    tiers: [{ tier: "master", total: 3, done: 1, pct: 33 }],
  },
  my_records: [
    {
      task_id: 11,
      name: "烧糊涂·10GB 上传挑战",
      status: 0,
      claimed_at: "2026-10-02T14:21:00Z",
      settled_at: null,
    },
    {
      task_id: 12,
      name: "字幕贡献·已完成",
      status: 1,
      claimed_at: "2026-09-18T09:30:00Z",
      settled_at: "2026-10-02T00:00:00Z",
    },
    {
      task_id: 13,
      name: "高烧·已失败",
      status: 2,
      claimed_at: "2026-09-05T11:12:00Z",
      settled_at: "2026-10-05T00:00:00Z",
    },
  ],
};

function stubFetch() {
  vi.stubGlobal(
    "fetch",
    vi.fn(async (input: RequestInfo | URL) => {
      const url = String(input);
      const data = url.includes("/tasks/overview")
        ? OV
        : url.includes("/api/v1/tasks")
          ? TASKS
          : {};
      return {
        ok: true,
        status: 200,
        headers: { get: () => "application/json" },
        json: async () => ({ code: 0, message: "ok", data, request_id: "t" }),
      } as unknown as Response;
    }),
  );
}

function mount() {
  return render(
    <LocaleProvider dict={zhCN} locale="zh-CN">
      <TaskBoard sparkBalance={8420} />
    </LocaleProvider>,
  );
}

async function mounted() {
  const utils = mount();
  // 等两个接口都落地：档位卡副标题来自 /tasks，动态里的人名来自
  // /tasks/overview。不能等「上传加速卡」——它在未选中的标签面板里，
  // 按本次改动的语义本就不该出现在 DOM 中。
  await waitFor(() => {
    expect(
      screen.getByText("20GB 上传挑战 · 大师级"),
    ).toBeInTheDocument();
    expect(screen.getByText("青柠")).toBeInTheDocument();
  });
  return utils;
}

beforeEach(() => stubFetch());
afterEach(() => vi.unstubAllGlobals());

describe("任务页两组标签", () => {
  it("默认只展开「规则」与「可领取任务」，另两块不进 DOM", async () => {
    await mounted();
    // 两组标签各展开一个面板
    expect(screen.getAllByRole("tabpanel")).toHaveLength(2);
    // 规则面板在
    expect(screen.getByText(t.rules[0])).toBeInTheDocument();
    // 可领取面板在（档位卡 + 名额说明）
    expect(screen.getByText("20GB 上传挑战 · 大师级")).toBeInTheDocument();
    expect(screen.getByText(/12 \/ 200/)).toBeInTheDocument();
    // 未选中的两个面板不得存在
    expect(screen.queryByText("上传加速卡")).toBeNull();
    expect(screen.queryByText("烧糊涂·10GB 上传挑战")).toBeNull();
  });

  it("切到「任务商店」：商店卡进来、任务卡让位，且不影响另一组", async () => {
    const user = userEvent.setup();
    await mounted();
    await user.click(screen.getByRole("tab", { name: t.shopTitle }));
    expect(screen.getByText("上传加速卡")).toBeInTheDocument();
    expect(screen.queryByText("20GB 上传挑战 · 大师级")).toBeNull();
    // 组一仍停在「规则」
    expect(screen.getByText(t.rules[0])).toBeInTheDocument();
  });

  it("切到「我的任务记录」：记录表进来，组二仍停在「可领取任务」", async () => {
    const user = userEvent.setup();
    await mounted();
    await user.click(screen.getByRole("tab", { name: t.historyTitle }));
    expect(screen.getByText("烧糊涂·10GB 上传挑战")).toBeInTheDocument();
    expect(screen.queryByText(t.rules[0])).toBeNull();
    expect(screen.getByText("20GB 上传挑战 · 大师级")).toBeInTheDocument();
  });

  it("面板内不得再有与标签同名的第二级标题", async () => {
    const { container } = await mounted();
    const labels = [t.rulesTitle, t.tiersTitle, t.historyTitle, t.shopTitle];
    const h2 = [...container.querySelectorAll("h2")].map((e) =>
      (e.textContent ?? "").trim(),
    );
    for (const l of labels) expect(h2).not.toContain(l);
    // 但标签本身仍带这些文案（标题层级没被删掉，只是唯一化）
    for (const l of labels)
      expect(screen.getByRole("tab", { name: l })).toBeInTheDocument();
  });

  it("aria 双向指认，且同组只有一个选中态", async () => {
    const { container } = await mounted();
    const panels = [...container.querySelectorAll('[role="tabpanel"]')];
    expect(panels).toHaveLength(2); // 每组各一个展开面板
    for (const tab of container.querySelectorAll('[role="tab"]')) {
      const sel = tab.getAttribute("aria-selected");
      const controls = tab.getAttribute("aria-controls");
      const target = controls
        ? container.querySelector(`#${CSS.escape(controls)}`)
        : null;
      if (sel === "true") {
        expect(target).not.toBeNull();
        expect(target?.getAttribute("aria-labelledby")).toBe(tab.id);
      } else {
        expect(target).toBeNull(); // 未选中的面板不该在 DOM 里
      }
    }
    for (const bar of container.querySelectorAll('[role="tablist"]'))
      expect(bar.querySelectorAll('[aria-selected="true"]')).toHaveLength(1);
  });

  it("方向键换标签：→ 从规则走到记录，← 走回来", async () => {
    const user = userEvent.setup();
    await mounted();
    const rulesTab = screen.getByRole("tab", { name: t.rulesTitle });
    rulesTab.focus();
    await user.keyboard("{ArrowRight}");
    expect(screen.getByText("烧糊涂·10GB 上传挑战")).toBeInTheDocument();
    await user.keyboard("{ArrowLeft}");
    expect(screen.getByText(t.rules[0])).toBeInTheDocument();
  });

  it("最新动态与任务统计不被收进标签（样图里它们是独立网格）", async () => {
    const { container } = await mounted();
    const grid = container.querySelector(".task-dashboard-grid");
    expect(grid).not.toBeNull();
    const heads = within(grid as HTMLElement)
      .getAllByRole("heading")
      .map((h) => (h.textContent ?? "").trim());
    expect(heads).toContain(t.feedTitle);
    expect(heads).toContain(t.statsTitle);
  });
  it("记录表三种结局各挂各的状态类（同色同重是这次要修的洞）", async () => {
    const user = userEvent.setup();
    const { container } = await mounted();
    await user.click(screen.getByRole("tab", { name: t.historyTitle }));
    const mods = [...container.querySelectorAll(".task-history__st")].map(
      (e) => e.className,
    );
    expect(mods).toHaveLength(3);
    expect(mods.join("|")).toContain("task-history__st--ongoing");
    expect(mods.join("|")).toContain("task-history__st--done");
    expect(mods.join("|")).toContain("task-history__st--failed");
  });
});
