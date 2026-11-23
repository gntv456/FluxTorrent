"use client";

import { useRef, useState } from "react";
import { SwTenModal } from "@/components/game/sw-panels";
import { drawWheelMany } from "@/lib/wheel-batch";
import { wheelPrizeName } from "@/lib/wheel-prize";
import type { WheelDrawResult } from "@/lib/wheel-batch";

/**
 * 四入口一排（样图定稿）：GO / 十连 / 百连 / 一直抽。
 *
 * 连抽不是后端批量端点，而是「同一发令枪抠 N 下」——每发独立幂等键、
 * 独立限次计数，服务端单局纪律不变。因此这里按「当前额度与余额」把
 * 请求发数封顶：额度只剩 12 局时，百连就是抽 12 局并在浮层里说清楚，
 * 不发 100 个必然 429 的请求。
 */
export function WheelCtas({
  ticket,
  balance,
  limitLeft,
  disabled,
  auto,
  onGo,
  onToggleAuto,
  onDone,
  onBusyChange,
  describeError,
  labels,
}: {
  ticket: number;
  balance: number;
  limitLeft: number;
  disabled: boolean;
  auto: boolean;
  onGo: () => void;
  onToggleAuto: () => void;
  onDone: () => void;
  onBusyChange?: (busy: boolean) => void;
  describeError: (e: unknown) => string;
  labels: {
    go: string;
    ten: string;
    hundred: string;
    auto: string;
    autoStop: string;
    batchTitle: string;
    batchTotal: string;
    partial: string;
    capped: string;
  };
}) {
  const [busy, setBusy] = useState(false);
  // state 在同一事件循环里来不及刷新，极快双击会绕过 busy；ref 是真正的闸。
  const busyRef = useRef(false);
  const [modal, setModal] = useState<{
    title: string;
    rows: { label: string; net: number }[];
    total: number;
    note: string;
  } | null>(null);

  const afford = ticket > 0 ? Math.floor(balance / ticket) : 0;
  const avail = Math.max(0, Math.min(limitLeft, afford));

  async function run(count: number) {
    if (busyRef.current || disabled) return;
    const n = Math.min(count, avail);
    if (n <= 0) return;
    busyRef.current = true;
    setBusy(true);
    onBusyChange?.(true);
    try {
      const out = await drawWheelMany<WheelDrawResult>(n);
      onDone();
      if (out.results.length === 0) {
        if (out.error) {
          setModal({
            title: labels.batchTitle.replace("{n}", String(n)),
            rows: [],
            total: 0,
            note: describeError(out.error),
          });
        }
        return;
      }
      const rows = out.results.slice(0, 20).map((r) => ({
        label: wheelPrizeName(r),
        net: r.net,
      }));
      const total = out.results.reduce((s, r) => s + r.net, 0);
      const notes: string[] = [];
      if (n < count) {
        notes.push(labels.capped.replace("{n}", String(n)));
      }
      if (out.results.length > rows.length) {
        notes.push(
          labels.partial.replace(
            "{n}",
            String(out.results.length - rows.length),
          ),
        );
      }
      if (out.error) notes.push(describeError(out.error));
      setModal({
        title: labels.batchTitle.replace("{n}", String(n)),
        rows,
        total,
        note: notes.join(" · "),
      });
    } finally {
      busyRef.current = false;
      setBusy(false);
      onBusyChange?.(false);
    }
  }

  const off = disabled || busy;
  const canBatch = !off && avail > 0;

  return (
    <>
      <div className="wp-cta-row">
        <button
          type="button"
          className="sw-cta cta-gold"
          onClick={() => {
            if (!busyRef.current) onGo();
          }}
          disabled={off || avail <= 0}
        >
          {labels.go}
        </button>
        <button
          type="button"
          className="wp-chip"
          onClick={() => void run(10)}
          disabled={!canBatch}
          title={labels.batchTitle.replace(
            "{n}",
            String(Math.min(10, avail)),
          )}
        >
          {labels.ten}
        </button>
        <button
          type="button"
          className="wp-chip purple"
          onClick={() => void run(100)}
          disabled={!canBatch}
          title={labels.batchTitle.replace(
            "{n}",
            String(Math.min(100, avail)),
          )}
        >
          {labels.hundred}
        </button>
        <button
          type="button"
          className={`wp-chip auto${auto ? " on" : ""}`}
          onClick={onToggleAuto}
          disabled={auto ? false : off || avail <= 0}
        >
          <span className="wp-auto-ic" aria-hidden>
            {auto ? "⏸" : "▶"}
          </span>
          {auto ? labels.autoStop : labels.auto}
        </button>
      </div>
      <SwTenModal
        open={modal !== null}
        title={modal?.title ?? ""}
        rows={modal?.rows ?? []}
        totalNet={modal?.total}
        note={modal?.note}
        totalLabel={labels.batchTotal}
        onClose={() => setModal(null)}
      />
    </>
  );
}
