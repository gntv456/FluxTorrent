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
 *
 *  2026-11-13 修：确认钮现 await run，期间 busy 禁用且保留弹窗；失败就地
 *  报错（run 抛错或返回 false），只有成功才 close()。旧写法先 close() 再
 *  void run(...)，失败时框已关、错误无处可播。
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
  /** 兼容旧调用：内部 busy 由状态机自管，此字段不再驱动禁用 */
  busy?: boolean;
  /** run 返回 false 或抛错＝失败：保留弹窗、就地显示错误 */
  errorText?: string;
  /** 失败回调（可选）：调用方可自行 toast / 打点 */
  onError?: (e: unknown) => void;
  run: (
    text: string,
    checked: boolean,
  ) => void | boolean | Promise<void | boolean>;
}

export function useConfirm() {
  const [ask, setAsk] = useState<ConfirmAsk | null>(null);
  const [text, setText] = useState("");
  const [checked, setChecked] = useState(false);
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const open = useCallback((a: ConfirmAsk) => {
    setText(a.initial ?? "");
    setChecked(a.check?.initial ?? false);
    setBusy(false);
    setError(null);
    setAsk(a);
  }, []);
  const close = useCallback(() => {
    setAsk(null);
    setBusy(false);
    setError(null);
  }, []);
  return {
    ask, text, setText, checked, setChecked,
    open, close, busy, setBusy, error, setError,
  };
}

export function AskDialog({
  ask,
  text,
  setText,
  checked,
  setChecked,
  close,
  busy,
  setBusy,
  error,
  setError,
}: {
  ask: ReturnType<typeof useConfirm>["ask"];
  text: string;
  setText: (v: string) => void;
  checked?: boolean;
  setChecked?: (v: boolean) => void;
  close: () => void;
  busy: boolean;
  setBusy: (v: boolean) => void;
  error: string | null;
  setError: (v: string | null) => void;
}) {
  if (!ask) return null;
  const onConfirm = async () => {
    if (busy) return;
    setBusy(true);
    setError(null);
    try {
      const r = await ask.run(text, !!checked);
      if (r === false) {
        setError(ask.errorText ?? "");
        ask.onError?.(undefined);
      } else {
        close();
      }
    } catch (e) {
      setError(
        ask.errorText ??
          (e instanceof Error ? e.message : String(e)),
      );
      ask.onError?.(e);
    } finally {
      setBusy(false);
    }
  };
  return (
    <ConfirmDialog
      open
      onClose={close}
      onConfirm={onConfirm}
      tone={ask.tone}
      title={ask.title}
      message={ask.message}
      requireText={ask.requireText}
      confirmLabel={ask.confirmLabel}
      busy={busy}
    >
      {error ? (
        <p
          role="alert"
          className="text-sm text-[var(--danger-text)]"
        >
          {error}
        </p>
      ) : null}
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
            className="min-h-11 rounded-[var(--r-sm)] border
              border-line bg-[var(--surface-card)] px-3 text-sm"
            data-autofocus
          />
        </label>
      ) : null}
    </ConfirmDialog>
  );
}
