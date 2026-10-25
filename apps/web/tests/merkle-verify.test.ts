import { describe, expect, it } from "vitest";

import { leafHash, verifyLeaf, type ProofStep } from "@/lib/merkle";

/** 与 crates/flux-shared/src/merkle.rs 的 reference_vector 同一组向量。 */
const PRE = [
  "flux-ledger-v1|2026-10-20|1|7|5|3:foo|-|-|100|" +
    "2026-10-20T01:00:00.000000Z",
  "flux-ledger-v1|2026-10-20|2|7|-2|3:bar|task|9|98|" +
    "2026-10-20T02:00:00.000000Z",
  "flux-ledger-v1|2026-10-20|3|8|1|4:seed|-|-|50|" +
    "2026-10-20T03:00:00.000000Z",
];

const LEAF = [
  "0bb45b817cc68f86ea9bb5c95ff7c2bcd1b005b589b4d8df4b97bdff6fd25d23",
  "8bac5a46c36a43e168b46f60e9281cd287124dd4d144ae3eeabb3ca5d93dc874",
  "10e8c966acb183c33ee8f5c22213b1ab05646573dfa750597fc8ff94e2c7fe9a",
];

const ROOT =
  "1fa855f2ba392f4d5a4cea81cb83b963777ffe23d2277ff60a00287b259d0b48";

const PROOF2: ProofStep[] = [
  { position: "right", hash: LEAF[2] },
  {
    position: "left",
    hash: "8f3a28c8040a08b1ce609c389e87e20d80b786d242a317bc5f1d5f39db7754e7",
  },
];

describe("浏览器侧 Merkle 复算", () => {
  it("参考向量与后端逐字节一致", async () => {
    for (let i = 0; i < PRE.length; i++) {
      expect(await leafHash(PRE[i])).toBe(LEAF[i]);
    }
    expect(await verifyLeaf(PRE[2], LEAF[2], PROOF2, ROOT)).toBe(true);
  });

  it("原文、兄弟哈希、树根任一被改都失败", async () => {
    expect(await verifyLeaf(PRE[2] + "x", LEAF[2], PROOF2, ROOT)).toBe(
      false,
    );
    const badProof: ProofStep[] = [
      PROOF2[0],
      { position: "left", hash: LEAF[1] },
    ];
    expect(await verifyLeaf(PRE[2], LEAF[2], badProof, ROOT)).toBe(false);
    expect(await verifyLeaf(PRE[2], LEAF[2], PROOF2, LEAF[0])).toBe(
      false,
    );
  });

  it("未知的拼接方向不被默认接受", async () => {
    const bad = [
      { position: "up", hash: LEAF[2] },
    ] as unknown as ProofStep[];
    expect(await verifyLeaf(PRE[2], LEAF[2], bad, ROOT)).toBe(false);
  });
});
