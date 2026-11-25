import { describe, expect, it } from "vitest";
import {
  claimFormVisible,
  prizeStatusText,
  type Detail,
} from "../app/(main)/games/lottery/detail";

const t: Record<string, string> = {
  statusPending: "待发放",
  statusFulfilled: "已发放",
  statusSatisfied: "已拥有",
  statusFailed: "失败",
  statusClaimed: "待审核",
  claimApproved: "已通过",
  claimSubmitted: "已提交",
  claimRejected: "已驳回：{reason}",
  claimExpired: "已过期",
  claimToSubmit: "待提交领取资料",
};

type Prize = NonNullable<Detail["myPrize"]>;
function prize(patch: Partial<Prize> = {}): Prize {
  return {
    winnerId: 1,
    prizeName: "自定义奖",
    slotNo: 1,
    status: "satisfied",
    rewardKind: "custom",
    claimStatus: null,
    rejectReason: null,
    submission: null,
    ...patch,
  };
}

describe("固定抽签·自定义奖领取入口", () => {
  it("履约终态为 satisfied 的自定义奖可以提交领取资料", () => {
    const p = prize();
    expect(claimFormVisible(p)).toBe(true);
    expect(prizeStatusText(p, t)).toBe("待提交领取资料");
  });

  it("已通过审核后不能再重交覆盖结果", () => {
    const p = prize({ claimStatus: "approved" });
    expect(claimFormVisible(p)).toBe(false);
    expect(prizeStatusText(p, t)).toBe("已通过");
  });

  it("待审与被驳回分别显示对应口径", () => {
    expect(prizeStatusText(prize({ claimStatus: "pending_review" }), t))
      .toBe("已提交");
    expect(
      prizeStatusText(
        prize({ claimStatus: "rejected", rejectReason: "信息不全" }),
        t
      )
    ).toBe("已驳回：信息不全");
  });

  it("超期显示过期且不再开放提交", () => {
    const p = prize({ claimStatus: "expired", status: "failed" });
    expect(claimFormVisible(p)).toBe(false);
    expect(prizeStatusText(p, t)).toBe("已过期");
  });

  it("发放类奖品不出现领取表单", () => {
    expect(claimFormVisible(prize({ rewardKind: "spark" }))).toBe(false);
    expect(prizeStatusText(prize({ rewardKind: "spark" }), t))
      .toBe("已拥有");
  });
});
