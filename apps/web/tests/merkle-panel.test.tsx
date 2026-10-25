import { describe, expect, it, vi, beforeEach } from "vitest";
import { render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";

import { api } from "@/lib/api-client";
import { MerklePanel } from "@/app/(main)/my-spark/_parts/merkle-panel";

const PRE =
  "flux-ledger-v1|2026-10-20|3|8|1|4:seed|-|-|50|" +
  "2026-10-20T03:00:00.000000Z";
const LEAF =
  "10e8c966acb183c33ee8f5c22213b1ab05646573dfa750597fc8ff94e2c7fe9a";
const ROOT =
  "1fa855f2ba392f4d5a4cea81cb83b963777ffe23d2277ff60a00287b259d0b48";
const NODE0 =
  "8f3a28c8040a08b1ce609c389e87e20d80b786d242a317bc5f1d5f39db7754e7";

function manifest(preimage = PRE) {
  return {
    built: true,
    date: "2026-10-20",
    timezone: "Asia/Shanghai",
    algorithm: "sha256-merkle-v1",
    anchoring: "none",
    root: ROOT,
    leaf_count: 3,
    user_count: 2,
    built_at: "2026-10-21T00:05:00Z",
    included: true,
    rows: [
      {
        idx: 2,
        ledger_id: 3,
        amount: 1,
        kind: "seeding_reward",
        created_at: "2026-10-20T03:00:00.000000Z",
        leaf: LEAF,
        preimage,
        proof: [
          { position: "right", hash: LEAF },
          { position: "left", hash: NODE0 },
        ],
      },
    ],
  };
}

describe("每日结算凭证面板", () => {
  beforeEach(() => vi.restoreAllMocks());

  it("真实渲染后可在浏览器复算通过", async () => {
    vi.spyOn(api, "get").mockImplementation(
      async () => manifest() as never,
    );
    render(<MerklePanel />);
    await userEvent.click(
      await screen.findByRole("button", { name: "在浏览器复算" }),
    );
    expect(
      await screen.findByText(/复算通过：你的 1 条记录/),
    ).toBeInTheDocument();
  });

  it("叶子原文被改时界面明确报未通过", async () => {
    vi.spyOn(api, "get").mockImplementation(
      async () => manifest(PRE + "tampered") as never,
    );
    render(<MerklePanel />);
    await userEvent.click(
      await screen.findByRole("button", { name: "在浏览器复算" }),
    );
    expect(
      await screen.findByText(/复算未通过/),
    ).toBeInTheDocument();
  });

  it("未封存日显示提示而不是空表", async () => {
    vi.spyOn(api, "get").mockImplementation(
      async () =>
        ({
          built: false,
          date: "2026-10-21",
          timezone: "Asia/Shanghai",
          algorithm: "sha256-merkle-v1",
          anchoring: "none",
          rows: [],
        }) as never,
    );
    render(<MerklePanel />);
    expect(
      await screen.findByText("该日尚未封存（每日结算后生成）。"),
    ).toBeInTheDocument();
  });
});
