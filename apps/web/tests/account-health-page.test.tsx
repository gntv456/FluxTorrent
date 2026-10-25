import { describe, it, expect, vi, beforeEach } from "vitest";
import { render, screen, waitFor } from "@testing-library/react";
import userEvent from "@testing-library/user-event";

const refresh = vi.fn();
vi.mock("next/headers", () => ({
  cookies: async () => ({
    get: (name: string) =>
      name === "flux_token" ? { value: "test-token" } : undefined,
  }),
}));
vi.mock("next/navigation", () => ({
  useRouter: () => ({ refresh }),
}));
vi.mock("@/i18n/server", () => ({
  getDict: async () => ({
    dict: {
      my: { title: "我的" },
      readState: { failTitle: "取数失败", failDesc: "稍后重试" },
      common: { networkError: "网络异常" },
    },
    locale: "zh-CN",
    currency: "魔力",
  }),
}));

import { api } from "@/lib/api-client";
import { accountHealthCopy } from "@/i18n/account-health";
import AccountHealthPage from "@/app/(main)/account-health/page";
import { AppealForm } from "@/app/(main)/account-health/_parts/appeal-form";
import type { MyRisk } from "@/app/(main)/account-health/_parts/risk-card";
import { RiskCard } from "@/app/(main)/account-health/_parts/risk-card";
import type {
  Onboarding,
} from "@/app/(main)/account-health/_parts/onboarding-card";
import type {
  SitePolicy,
} from "@/app/(main)/account-health/_parts/policy-card";

const risk: MyRisk = {
  score: 42,
  band: "watch",
  ruleset_ver: 3,
  updated_at: "2026-10-20T00:00:00Z",
  decay_half_life_days: 14,
  reasons: [
    {
      code: "ghost_seed",
      hits: 2,
      torrent_ids: [9],
      first_seen: "2026-10-01T00:00:00Z",
      last_seen: "2026-10-20T00:00:00Z",
      detail: "无上传数据的做种记录",
    },
  ],
  open_case: {
    id: 7,
    status: 0,
    reason: "多日幽灵做种",
    priority: 2,
    risk_score: 42,
    created_at: "2026-10-19T00:00:00Z",
  },
  latest_appeal: {
    id: 3,
    case_id: 7,
    status: "open",
    outcome: null,
    result_note: null,
    created_at: "2026-10-20T00:00:00Z",
  },
  appeal_available: false,
  case_pending: true,
  open_appeals: 1,
  appeal_limit: 3,
  appeal_path: "/account-health#appeal",
};

const onboarding: Onboarding = {
  steps: [
    { key: "profile", done: true, href: "/my?tab=personal" },
    { key: "first_download", done: true, href: "/torrents" },
    { key: "passkey", done: false, href: "/my?tab=security" },
  ],
  done: 2,
  total: 3,
  completed: false,
};

const policy: SitePolicy = {
  schema: 1,
  version: "abc12345",
  updated_at: null,
  hr: {
    seed_hours: 48,
    days: 14,
    warn_enable: true,
    prewarn_hours: 0,
    violation_limit: 3,
    warn_base_days: 7,
    warn_max_shift_days: 365,
  },
  risk_decay_days: 14,
  risk_explanation_path: "/account-health",
  lottery_verification_path: "/api/v1/forums/lottery/{topic_id}/fairness",
  lottery_algorithm: "sha256-commit-reveal-v1",
};

beforeEach(() => {
  vi.restoreAllMocks();
  refresh.mockReset();
});

describe("账户健康页（RSC 渲染）", () => {
  it("三路取数都能落成页面文案", async () => {
    vi.spyOn(api, "get").mockImplementation(async (path: string) => {
      if (path === "/api/v1/me/risk") return risk;
      if (path === "/api/v1/me/onboarding") return onboarding;
      if (path === "/api/v1/system/policies") return policy;
      throw new Error(`unexpected ${path}`);
    });
    render(await AccountHealthPage());
    expect(screen.getByRole("heading", { name: "账户健康" }))
      .toBeInTheDocument();
    expect(screen.getByText("观察")).toBeInTheDocument();
    expect(screen.getByText("42")).toBeInTheDocument();
    expect(screen.getByText("幽灵做种（无实际数据上传）"))
      .toBeInTheDocument();
    expect(screen.getByText("2 次")).toBeInTheDocument();
    expect(screen.getByText("案件 #7")).toBeInTheDocument();
    expect(
      screen.getByText("该案件已有未结申诉，请等待处理结果。"),
    ).toBeInTheDocument();
    expect(screen.getByText("已完成 2/3")).toBeInTheDocument();
    expect(screen.getByText("完成首次下载")).toBeInTheDocument();
    expect(
      screen.getByText("H&R 站点默认：做种 48 小时 / 14 天"),
    ).toBeInTheDocument();
    expect(screen.getByText("策略版本 abc12345"))
      .toBeInTheDocument();
  });

  it("单口失败只降级对应卡片，不把整页演成失败", async () => {
    vi.spyOn(api, "get").mockImplementation(async (path: string) => {
      if (path === "/api/v1/me/onboarding") return onboarding;
      if (path === "/api/v1/system/policies") return policy;
      throw new Error("down");
    });
    render(await AccountHealthPage());
    expect(screen.getByText("新手清单")).toBeInTheDocument();
    expect(screen.getAllByText("取数失败").length).toBeGreaterThan(0);
    expect(screen.getByText("策略版本 abc12345"))
      .toBeInTheDocument();
    expect(screen.queryByText("风险与证据")).not.toBeInTheDocument();
  });

  it("会员两口都失败但策略可取时仍展示策略", async () => {
    vi.spyOn(api, "get").mockImplementation(async (path: string) => {
      if (path === "/api/v1/system/policies") return policy;
      throw new Error("down");
    });
    render(await AccountHealthPage());
    expect(screen.getByText("策略版本 abc12345"))
      .toBeInTheDocument();
    expect(screen.queryByText("风险与证据")).not.toBeInTheDocument();
    expect(screen.queryByText("新手清单")).not.toBeInTheDocument();
    expect(screen.getAllByText("取数失败").length)
      .toBeGreaterThanOrEqual(2);
  });

  it("顺延上限 0 渲染为不封顶而不是 0 天", async () => {
    vi.spyOn(api, "get").mockImplementation(async (path: string) => {
      if (path === "/api/v1/me/risk") return risk;
      if (path === "/api/v1/me/onboarding") return onboarding;
      if (path === "/api/v1/system/policies") {
        return {
          ...policy,
          hr: { ...policy.hr, warn_max_shift_days: 0 },
        };
      }
      throw new Error(`unexpected ${path}`);
    });
    render(await AccountHealthPage());
    expect(screen.getByText("警告期 7 天；反复违规顺延不封顶"))
      .toBeInTheDocument();
    expect(screen.queryByText(/最长可延长至 0 天/))
      .not.toBeInTheDocument();
  });

  it("当前案件有未结申诉时不被别的案件驳回结论洗掉", () => {
    render(
      <RiskCard
        t={accountHealthCopy["zh-CN"]}
        locale="zh-CN"
        risk={{
          ...risk,
          latest_appeal: {
            id: 9,
            case_id: 8,
            status: "rejected",
            outcome: "upheld",
            result_note: null,
            created_at: "2026-10-20T00:00:00Z",
          },
        }}
      />,
    );
    expect(
      screen.getByText("该案件已有未结申诉，请等待处理结果。"),
    ).toBeInTheDocument();
  });
});

describe("申诉表单", () => {
  it("提交 case_id 与正文，成功后收起并提示", async () => {
    const post = vi
      .spyOn(api, "post")
      .mockResolvedValue({ id: 11 });
    const user = userEvent.setup();
    render(
      <AppealForm
        caseId={7}
        copy={accountHealthCopy["zh-CN"].appeal}
      />,
    );
    await user.type(
      screen.getByLabelText("申诉说明"),
      "我有完整的做种日志可以核对",
    );
    await user.click(
      screen.getByRole("button", { name: "提交申诉" }),
    );
    await waitFor(() =>
      expect(post).toHaveBeenCalledWith("/api/v1/cheat-appeals", {
        case_id: 7,
        body: "我有完整的做种日志可以核对",
      }),
    );
    expect(await screen.findByRole("status"))
      .toHaveTextContent("申诉已提交");
    expect(refresh).toHaveBeenCalled();
  });
});
