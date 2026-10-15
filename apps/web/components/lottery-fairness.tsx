"use client";

/**
 * 抽奖公平性验证面板（0382 的前端面）。
 *
 * commit-reveal 的价值全在「用户能自己验」。这个面板把后端给的
 * 材料原样摊开，并用浏览器 WebCrypto **现场复算一遍**——
 * 用户看到的不是「本站说它公平」，而是「我自己算了一遍，结果对得上」。
 *
 * 为什么值得在浏览器里再算一次：
 *   后端已经自验过（`verified` 字段），但那仍是「本站说本站对」。
 *   让用户在自己的机器上跑一遍同一段算法，才真正把信任交出去。
 *
 * 注意：WebCrypto 的 SHA-256/HMAC 需要 `crypto.subtle`，只在安全上下文
 * （https / localhost）可用。故降级路径是「只展示材料 + 后端结论」，
 * 而不是报错——验证页不可用比不能验证更糟。
 */

import { useCallback, useEffect, useState } from "react";

import { api } from "@/lib/api-client";
import { useI18n } from "@/i18n/client";
import { StatePanel } from "@/components/ui/state-panel";

interface Fairness {
  exists: boolean;
  stage?: "open" | "drawn";
  commit?: string;
  proof?: {
    round_id: string;
    seed: string;
    tickets_root: string;
    rand: string;
    winner_ticket_hashes: string[];
  };
  ticket_count?: number;
  winners_claimed?: number;
  verified?: boolean;
  verify_note?: string;
  algorithm?: Record<string, string>;
  note?: string;
}

async function sha256Hex(text: string): Promise<string> {
  const buf = new TextEncoder().encode(text);
  const digest = await crypto.subtle.digest("SHA-256", buf);
  return [...new Uint8Array(digest)]
    .map((b) => b.toString(16).padStart(2, "0"))
    .join("");
}

async function hmacHex(
  keyHex: string,
  msg: string,
): Promise<string> {
  const key = new TextEncoder().encode(keyHex);
  const k = await crypto.subtle.importKey(
    "raw",
    key,
    { name: "HMAC", hash: "SHA-256" },
    false,
    ["sign"],
  );
  const sig = await crypto.subtle.sign(
    "HMAC",
    k,
    new TextEncoder().encode(msg),
  );
  return [...new Uint8Array(sig)]
    .map((b) => b.toString(16).padStart(2, "0"))
    .join("");
}

export function LotteryFairness({ topicId }: { topicId: number }) {
  const { dict } = useI18n();
  const [data, setData] = useState<Fairness | null>(null);
  const [err, setErr] = useState<string | null>(null);
  /** 浏览器本地复算结论；null = 还没算 / 算不了 */
  const [local, setLocal] = useState<boolean | null>(null);

  const load = useCallback(async () => {
    try {
      const f = await api.get<Fairness>(
        `/api/v1/forums/lottery/${topicId}/fairness`,
      );
      setData(f);
      setErr(null);
      // 已开奖才现场复算；开售中没有 seed 可算
      if (f.stage === "drawn" && f.proof) {
        setLocal(null);
        try {
          const expectCommit = await sha256Hex(f.proof.seed);
          const expectRand = await hmacHex(
            f.proof.seed,
            `${f.proof.round_id}|${f.proof.tickets_root}`,
          );
          setLocal(
            expectCommit === f.commit && expectRand === f.proof.rand,
          );
        } catch {
          // 非安全上下文：保持 null，落到「材料 + 后端结论」的降级展示
          setLocal(null);
        }
      }
    } catch (e) {
      setErr(e instanceof Error ? e.message : String(e));
    }
  }, [topicId]);

  useEffect(() => {
    void load();
  }, [load]);

  const box =
    "rounded-lg border border-[var(--border-soft)] p-3 text-xs";

  return (
    <StatePanel
      loading={data === null && err === null}
      error={err}
      empty={data !== null && !data.exists}
      onRetry={() => void load()}
    >
      {data && data.exists && (
        <div className="flex flex-col gap-3">
          <div className="flex items-center justify-between gap-3">
            <span className="text-sm font-medium
                             text-[var(--text-strong)]">
              {data.stage === "drawn" ? dict.lotteryFair.drawn : dict.lotteryFair.ongoing}
            </span>
            {data.verified !== undefined && (
              <span
                className={
                  data.verified
                    ? "text-xs text-[var(--accent)]"
                    : "text-xs text-[var(--text-faint)]"
                }
              >
                {data.verified ? dict.lotteryFair.serverOk : dict.lotteryFair.serverFail}
              </span>
            )}
          </div>

          {data.stage === "open" ? (
            <div className={box}>
              <p className="text-[var(--text-muted)]">{data.note}</p>
              <p className={box + " mt-2 break-all font-mono"}>
                commit: {data.commit}
              </p>
            </div>
          ) : (
            <div className="flex flex-col gap-2">
              {local !== null && (
                <p className="text-xs">
                  <span
                    className={
                      local
                        ? "text-[var(--accent)]"
                        : "text-[var(--text-faint)]"
                    }
                  >
                    {local
                      ? dict.lotteryFair.browserOk
                      : dict.lotteryFair.browserFail}
                  </span>
                </p>
              )}
              <div className={box}>
                <p className="mb-1 text-[var(--text-muted)]">
                  票据 {data.ticket_count ?? 0} 张 · 中奖{" "}
                  {data.winners_claimed ?? 0} 名
                </p>
                <p className="break-all font-mono">
                  seed: {data.proof?.seed}
                </p>
                <p className="break-all font-mono">
                  tickets_root: {data.proof?.tickets_root}
                </p>
                <p className="break-all font-mono">
                  rand: {data.proof?.rand}
                </p>
              </div>
              {data.verify_note && (
                <p className="text-xs text-[var(--text-faint)]">
                  {data.verify_note}
                </p>
              )}
            </div>
          )}
        </div>
      )}
    </StatePanel>
  );
}
