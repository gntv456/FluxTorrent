"use client";

/**
 * 通用确认状态机（P2 清偿 B：window.confirm/prompt 退役的批量载体）。
 *
 * 一处声明、多处复用：把「确认后再执行」收敛成一个 ask 状态 + 渲染一个
 * ConfirmDialog。prompt 语义用 text 槽（children 输入框）表达，confirm
 * 语义直接挂 run。danger 判定由调用方按场景给。
 *
 *   const ask = useConfirm();
 *   ...
 *   ask.confirm({ title, message, tone: "danger", run: () => del(r) });
 *   ask.prompt({ title, label, initial, run: (text) => rename(r, text) });
 *   ...
 *   <AskDialog ask={ask} />
 *
 *  两个后补的槽（2026-10-16 弹窗普查）：
 *  · `requireText`——破坏性动作的「输入指定内容才解禁确认钮」，能力本来只在
 *    直挂 ConfirmDialog 的调用点用得上（全站 1 处），走 ask 状态机的都用不上；
 *  · `check`——一个复选槽。申诉受理要同时回答「是否联动解封」，旧写法是
 *    先提交再弹第二个框，第二次提交被后端 CAS 拒 ⇒ 那一步永远做不成。
 */

import { useCallback, useState } from "react";
import { ConfirmDialog } from "@/components/ui/confirm-dialog";

export interface ConfirmAsk {
  title: string;
  message?: string;
  tone?: "neutral" | "danger";
  /** prompt 槽：给出 label 即渲染输入框，run 收到输入文本 */
  label?: string;
  initial?: string;
  maxLength?: number;
  /** 删除类二次输入：用户原样输入这串才解禁确认钮（透传 ConfirmDialog） */
  requireText?: string;
  /** 复选槽：label 显示文案，run 的第二参收勾选状态 */
  check?: { label: string; initial?: boolean };
  /** 确认钮文案：不给时按 tone 回落（danger=删除/其余=确认）——批量通过等
   *  danger 但语义非删除的场景必须显式给（2026-10-21 种审员实测：批量通过
   *  的确认钮显示「删除」）。 */
  confirmLabel?: string;
  busy?: boolean;
  run: (text: string, checked: boolean) => void | Promise<void>;
}

export function useConfirm() {
  const [ask, setAsk] = useState<ConfirmAsk | null>(null);
  const [text, setText] = useState("");
  const [checked, setChecked] = useState(false);
  const open = useCallback((a: ConfirmAsk) => {
    setText(a.initial ?? "");
    setChecked(a.check?.initial ?? false);
    setAsk(a);
  }, []);
  const close = useCallback(() => setAsk(null), []);
  return { ask, text, setText, checked, setChecked, open, close };
}

export function AskDialog({
  ask,
  text,
  setText,
  checked,
  setChecked,
  close,
}: {
  ask: ReturnType<typeof useConfirm>["ask"];
  text: string;
  setText: (v: string) => void;
  checked?: boolean;
  setChecked?: (v: boolean) => void;
  close: () => void;
}) {
  if (!ask) return null;
  return (
    <ConfirmDialog
      open
      onClose={close}
      onConfirm={() => {
        close();
        void ask.run(text, !!checked);
      }}
      tone={ask.tone}
      title={ask.title}
      message={ask.message}
      requireText={ask.requireText}
      confirmLabel={ask.confirmLabel}
      busy={ask.busy}
    >
      {ask.check ? (
        <label className="flex items-center gap-2 text-sm">
          <input
            type="checkbox"
            checked={!!checked}
            onChange={(e) => setChecked?.(e.target.checked)}
          />
          <span>{ask.check.label}</span>
        </label>
      ) : null}
      {ask.label ? (
        <label className="flex flex-col gap-1 text-sm">
          <span>{ask.label}</span>
          <input
            value={text}
            onChange={(e) => setText(e.target.value)}
            maxLength={ask.maxLength}
            className="min-h-[40px] rounded-[var(--r-sm)] border
              border-line bg-[var(--surface-card)] px-3 text-sm"
            data-autofocus
          />
        </label>
      ) : null}
    </ConfirmDialog>
  );
}
