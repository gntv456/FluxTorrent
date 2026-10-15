"use client";

/**
 * 注册答题题库后台（0372 M5）：`?tool=regquiz`。
 * 题库 CRUD + 批量导入 + 近 N 天通过率。与「一次性邮箱域名」同属
 * 注册准入链（前置答题闸），但它是内容面而非拦截名单，故独立工具页。
 *
 * 出题口径：单选；选项 2-6 个；answer_index 0 基。判分票据由
 * /auth/quiz-challenge 签发（Redis 一次性），本页不改判分逻辑。
 * 表单与导入在 staff-tools-regquiz-parts.tsx（300 行门禁）。
 */

import { useCallback, useEffect, useState } from "react";
import { api } from "@/lib/api-client";
import { useI18n } from "@/i18n/client";
import { fmt } from "@/i18n/config";
import {
  RegQuizForm,
  RegQuizImport,
  useGuard,
  type ListResp,
  type QuizRow,
} from "./staff-tools-regquiz-parts";

export function StaffRegQuizPanel({
  flash,
}: {
  flash: (m: string) => void;
}) {
  const { dict } = useI18n();
  const t = dict.stafftools;
  const [data, setData] = useState<ListResp | null>(null);
  const [editRow, setEditRow] = useState<QuizRow | null>(null);
  const { run } = useGuard(flash);

  const load = useCallback(async () => {
    try {
      setData(await api.get<ListResp>("/api/v1/admin/reg-quiz?limit=500"));
    } catch {
      setData(null);
    }
  }, []);
  useEffect(() => {
    void load();
  }, [load]);

  function afterWrite() {
    setEditRow(null);
    void load();
  }

  function toggle(r: QuizRow) {
    void run(async () => {
      await api.post("/api/v1/admin/reg-quiz/update", {
        id: r.id,
        enabled: !r.enabled,
      });
      await load();
    });
  }

  function doDelete(id: number) {
    void run(async () => {
      await api.post("/api/v1/admin/reg-quiz/delete", { ids: [id] });
      await load();
    }, t.rqDeleted);
  }

  const st = data?.stats;
  const rate = st ? Math.round((st.pass_rate ?? 0) * 100) : 0;

  return (
    <>
      <RegQuizForm
        editRow={editRow}
        flash={flash}
        onDone={afterWrite}
        onCancel={() => setEditRow(null)}
      />

      <RegQuizImport flash={flash} onDone={() => void load()} />

      <section className="brand-panel mt-4 p-4">
        {st && (
          <p className="mb-3 text-xs text-sub">
            {fmt(t.rqStats, {
              total: data?.total ?? 0,
              days: st.days,
              attempts: st.attempts,
              rate,
            })}
          </p>
        )}
        <table className="nexus-table">
          <tbody>
            <tr>
              <td className="colhead">{t.rqColId}</td>
              <td className="colhead">{t.rqColCategory}</td>
              <td className="colhead">{t.rqColQuestion}</td>
              <td className="colhead">{t.rqColAnswer}</td>
              <td className="colhead">{t.rqColEnabled}</td>
              <td className="colhead text-right">{dict.cmgmt.colActions}</td>
            </tr>
            {(data?.items ?? []).map((r) => (
              <tr key={r.id}>
                <td className="num">{r.id}</td>
                <td>{r.category}</td>
                <td className="max-w-[360px]">{r.question}</td>
                <td>{(r.options ?? [])[r.answer_index] ?? "—"}</td>
                <td>{r.enabled ? t.rqOn : t.rqOff}</td>
                <td className="whitespace-nowrap text-right">
                  <button
                    className="cmgmt-act cmgmt-act--ok"
                    onClick={() => setEditRow(r)}
                  >
                    {t.rqEditBtn}
                  </button>
                  <button className="cmgmt-act" onClick={() => toggle(r)}>
                    {r.enabled ? t.rqOff : t.rqOn}
                  </button>
                  <button
                    className="cmgmt-act cmgmt-act--danger"
                    onClick={() => doDelete(r.id)}
                  >
                    {dict.cmgmt.btnDelete}
                  </button>
                </td>
              </tr>
            ))}
            {data !== null && (data.items ?? []).length === 0 && (
              <tr>
                <td colSpan={6} className="py-6 text-center text-sub">
                  {t.rqEmpty}
                </td>
              </tr>
            )}
          </tbody>
        </table>
      </section>
    </>
  );
}
