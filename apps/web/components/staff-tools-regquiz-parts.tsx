"use client";

/**
 * 注册答题题库后台·共享件（0372 M5）：类型 + 动作包装（useGuard）+
 * 新增/编辑表单 + 批量导入。列表主体留在 staff-tools-regquiz.tsx
 * （300 行门禁所致，模式同 staff-tools-security-*）。
 */

import { useCallback, useEffect, useState } from "react";
import { api, ApiError } from "@/lib/api-client";
import { useI18n } from "@/i18n/client";
import { fmt } from "@/i18n/config";

export interface QuizRow {
  id: number;
  category: string;
  question: string;
  options: string[];
  answer_index: number;
  explanation: string | null;
  enabled: boolean;
}

export interface Stats {
  days: number;
  attempts: number;
  passed: number;
  pass_rate: number;
}

export interface ListResp {
  items: QuizRow[];
  total: number;
  stats: Stats;
}

export interface ImportResp {
  added: number;
  skipped: number;
  invalid: number;
}

/** 统一动作包装：busy 互斥 + ApiError 归一到 flash。 */
export function useGuard(flash: (m: string) => void) {
  const { dict } = useI18n();
  const [busy, setBusy] = useState(false);
  const run = useCallback(
    async (fn: () => Promise<void>, ok?: string) => {
      setBusy(true);
      try {
        await fn();
        if (ok) flash(ok);
      } catch (e) {
        flash(e instanceof ApiError ? e.message : dict.common.networkError);
      } finally {
        setBusy(false);
      }
    },
    [flash, dict],
  );
  return { busy, run };
}

/** 新增（editRow=null）/ 编辑（editRow 非空）表单。 */
export function RegQuizForm({
  editRow,
  flash,
  onDone,
  onCancel,
}: {
  editRow: QuizRow | null;
  flash: (m: string) => void;
  onDone: () => void;
  onCancel: () => void;
}) {
  const { dict } = useI18n();
  const t = dict.stafftools;
  const { busy, run } = useGuard(flash);
  const [cat, setCat] = useState("");
  const [question, setQuestion] = useState("");
  const [opts, setOpts] = useState("");
  const [ans, setAns] = useState(0);
  const [expl, setExpl] = useState("");
  const [enabled, setEnabled] = useState(true);

  useEffect(() => {
    if (editRow === null) return;
    setCat(editRow.category);
    setQuestion(editRow.question);
    setOpts((editRow.options ?? []).join("\n"));
    setAns(editRow.answer_index);
    setExpl(editRow.explanation ?? "");
    setEnabled(editRow.enabled);
  }, [editRow]);

  const list = opts
    .split("\n")
    .map((s) => s.trim())
    .filter(Boolean);

  function save() {
    void run(async () => {
      const body = {
        category: cat.trim(),
        question: question.trim(),
        options: list,
        answer_index: ans,
        explanation: expl.trim() || null,
        enabled,
      };
      if (editRow === null) {
        await api.post("/api/v1/admin/reg-quiz", body);
      } else {
        await api.post("/api/v1/admin/reg-quiz/update", {
          id: editRow.id,
          ...body,
        });
      }
      setCat("");
      setQuestion("");
      setOpts("");
      setAns(0);
      setExpl("");
      onDone();
    }, t.rqSaved);
  }

  return (
    <section className="brand-panel p-4">
      <h2 className="mb-1 text-base font-bold text-ink">
        {editRow === null ? t.rqTitle : fmt(t.rqEditTitle, { id: editRow.id })}
      </h2>
      <p className="mb-2 text-xs text-sub">{t.rqNote}</p>
      <div className="cmgmt-form">
        <label>
          {t.rqCategory}
          <input
            value={cat}
            onChange={(e) => setCat(e.target.value)}
            placeholder={t.rqCategoryPh}
          />
        </label>
        <label>
          {t.rqQuestion}
          <textarea
            rows={3}
            value={question}
            onChange={(e) => setQuestion(e.target.value)}
          />
        </label>
        <label>
          {t.rqOptions}
          <textarea
            rows={5}
            value={opts}
            onChange={(e) => setOpts(e.target.value)}
          />
        </label>
        <label>
          {t.rqAnswer}
          <select
            value={ans}
            onChange={(e) => setAns(Number(e.target.value))}
          >
            {(list.length > 0 ? list : [t.rqNoOpt]).map((o, i) => (
              <option key={`${i}-${o.slice(0, 10)}`} value={i}>
                {i + 1}. {o}
              </option>
            ))}
          </select>
        </label>
        <label>
          {t.rqExplanation}
          <input value={expl} onChange={(e) => setExpl(e.target.value)} />
        </label>
        <label className="flex flex-row items-center gap-2">
          <input
            type="checkbox"
            checked={enabled}
            onChange={(e) => setEnabled(e.target.checked)}
          />
          {t.rqEnabled}
        </label>
        <div className="flex flex-wrap gap-2 self-start">
          <button
            className="brand-button"
            disabled={busy || question.trim().length < 4 || list.length < 2}
            onClick={save}
          >
            {editRow === null ? t.rqCreate : t.rqSave}
          </button>
          {editRow !== null && (
            <button className="brand-button" onClick={onCancel}>
              {t.rqCancel}
            </button>
          )}
        </div>
      </div>
    </section>
  );
}

/** 批量导入（JSON 数组，幂等：同题干跳过）。 */
export function RegQuizImport({
  flash,
  onDone,
}: {
  flash: (m: string) => void;
  onDone: () => void;
}) {
  const { dict } = useI18n();
  const t = dict.stafftools;
  const { busy, run } = useGuard(flash);
  const [text, setText] = useState("");

  function doImport() {
    const raw = text.trim();
    if (!raw) return;
    let questions: unknown;
    try {
      questions = JSON.parse(raw);
      if (!Array.isArray(questions)) throw new Error("not-array");
    } catch {
      flash(t.rqBadJson);
      return;
    }
    void run(async () => {
      const r = await api.post<ImportResp>(
        "/api/v1/admin/reg-quiz/import",
        { questions },
      );
      setText("");
      onDone();
      flash(
        fmt(t.rqImported, {
          added: r.added,
          skipped: r.skipped,
          invalid: r.invalid,
        }),
      );
    });
  }

  return (
    <section className="brand-panel mt-4 p-4">
      <h3 className="mb-2 text-sm font-bold">{t.rqImport}</h3>
      <div className="cmgmt-form">
        <label>
          {t.rqImportLabel}
          <textarea
            rows={4}
            value={text}
            onChange={(e) => setText(e.target.value)}
            placeholder={t.rqImportPh}
          />
        </label>
        <button
          className="brand-button self-start"
          disabled={busy || !text.trim()}
          onClick={doImport}
        >
          {t.rqImportBtn}
        </button>
      </div>
    </section>
  );
}
