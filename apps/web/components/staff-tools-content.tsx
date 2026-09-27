"use client";

import { BTN_SM_BOLD } from "@/lib/ui-classes";
import { ContentRulesTab } from "./staff-tools-content-rules";

import { useCallback, useEffect, useState } from "react";
import { api, ApiError } from "@/lib/api-client";
import { useI18n } from "@/i18n/client";
import type { ToolTab } from "@/components/staff-tools";
import { StaffCatsPanel } from "./staff-tools-content-cats";
import type {
  CatItem,
  FaqItem,
  PackApplyItem,
  RuleItem,
  TypePack,
} from "./staff-tools-content-shared";

/** 内容域面板（从 staff-tools.tsx 按域拆出，300 行门禁）：
 *  FAQ 管理（faq）/ 规则管理（rules）/ 分类管理（cats，含类型包切换）。
 *  分类/类型包拆至 ./staff-tools-content-cats.tsx；
 *  类型拆至 ./staff-tools-content-shared.ts。 */

export function StaffContentPanel({
  tab,
  flash,
}: {
  tab: ToolTab;
  flash: (m: string) => void;
}) {
  const { dict } = useI18n();
  const t = dict.stafftools;
  const [faqs, setFaqs] = useState<FaqItem[]>([]);
  const [rules, setRules] = useState<RuleItem[]>([]);
  const [cats, setCats] = useState<CatItem[]>([]);
  const [packs, setPacks] = useState<TypePack[]>([]);
  // 站型应用记录（G21：回看 + 回滚）
  const [applies, setApplies] = useState<PackApplyItem[]>([]);
  // 当前站型 code（site-profile）：类型包列表标亮「使用中」
  const [curSiteType, setCurSiteType] = useState<string>("");
  const [packMode, setPackMode] = useState<"replace" | "merge">("merge");
  const [busy, setBusy] = useState(false);

  // FAQ / 规则编辑器
  const [faqEdit, setFaqEdit] = useState<{
    id: number | null;
    question: string;
    answer: string;
  }>({ id: null, question: "", answer: "" });
  const [ruleEdit, setRuleEdit] = useState<{
    id: number | null;
    title: string;
    body: string;
  }>({ id: null, title: "", body: "" });
  const [catName, setCatName] = useState("");
  const [catParent, setCatParent] = useState<number | "">("");

  const load = useCallback(async () => {
    api
      .get<FaqItem[]>("/api/v1/faq")
      .then(setFaqs)
      .catch(() => setFaqs([]));
    api
      .get<RuleItem[]>("/api/v1/rules-content")
      .then(setRules)
      .catch(() => setRules([]));
    // 分类列表（管理口径，含每分类种子数）
    api
      .get<CatItem[]>("/api/v1/admin/categories")
      .then(setCats)
      .catch(() => setCats([]));
    api
      .get<TypePack[]>("/api/v1/admin/site-type-packs")
      .then(setPacks)
      .catch(() => setPacks([]));
    api
      .get<PackApplyItem[]>("/api/v1/admin/site-type-packs/applies")
      .then(setApplies)
      .catch(() => setApplies([]));
    api
      .get<{ site_type: string }>("/api/v1/site-profile")
      .then((p) => setCurSiteType(p.site_type ?? ""))
      .catch(() => {});
  }, []);
  useEffect(() => {
    load();
  }, [load]);

  async function guard(fn: () => Promise<void>, ok: string) {
    setBusy(true);
    try {
      await fn();
      flash(ok);
      await load();
    } catch (e) {
      flash(e instanceof ApiError ? e.message : dict.common.networkError);
    } finally {
      setBusy(false);
    }
  }

  return (
    <>
      {/* FAQ 管理 */}
      {tab === "faq" && (
        <>
          <section className="baozi-panel p-4">
            <h2 className="mb-3 text-base font-bold text-ink">
              {faqEdit.id === null ? t.faqNew : t.faqEdit}
            </h2>
            <div className="cmgmt-form">
              <label>
                {t.fldQuestion}
                <input
                  value={faqEdit.question}
                  onChange={(e) =>
                    setFaqEdit({ ...faqEdit, question: e.target.value })
                  }
                />
              </label>
              <label>
                {t.fldAnswer}
                <textarea
                  rows={4}
                  value={faqEdit.answer}
                  onChange={(e) =>
                    setFaqEdit({ ...faqEdit, answer: e.target.value })
                  }
                />
              </label>
              <div className="flex gap-2">
                <button
                  className="baozi-button"
                  disabled={busy || !faqEdit.question.trim()}
                  onClick={() =>
                    guard(async () => {
                      if (faqEdit.id === null)
                        await api.post("/api/v1/admin/faq", {
                          question: faqEdit.question,
                          answer: faqEdit.answer,
                        });
                      else
                        await api.put(`/api/v1/admin/faq/${faqEdit.id}`, {
                          question: faqEdit.question,
                          answer: faqEdit.answer,
                        });
                      setFaqEdit({ id: null, question: "", answer: "" });
                    }, t.saved)
                  }
                >
                  {t.btnSave}
                </button>
                {faqEdit.id !== null && (
                  <button
                    className={BTN_SM_BOLD}
                    onClick={() =>
                      setFaqEdit({ id: null, question: "", answer: "" })
                    }
                  >
                    {dict.cmgmt.btnCancel}
                  </button>
                )}
              </div>
            </div>
          </section>
          <table className="nexus-table">
            <tbody>
              <tr>
                <td className="colhead">{t.fldQuestion}</td>
                <td className="colhead text-right">{dict.cmgmt.colActions}</td>
              </tr>
              {faqs.map((f) => (
                <tr key={f.id}>
                  <td>{f.question}</td>
                  <td className="text-right">
                    <button
                      className="cmgmt-act"
                      onClick={() =>
                        setFaqEdit({
                          id: f.id,
                          question: f.question,
                          answer: f.answer,
                        })
                      }
                    >
                      {dict.cmgmt.btnEdit}
                    </button>
                    <button
                      className="cmgmt-act cmgmt-act--danger"
                      onClick={() =>
                        guard(async () => {
                          await api.del(`/api/v1/admin/faq/${f.id}`);
                        }, t.deleted)
                      }
                    >
                      {dict.cmgmt.btnDelete}
                    </button>
                  </td>
                </tr>
              ))}
            </tbody>
          </table>
        </>
      )}

      {/* 分类管理（拆至 ./staff-tools-content-cats.tsx） */}
      {tab === "rules" && (
        <ContentRulesTab
          rules={rules}
          ruleEdit={ruleEdit}
          setRuleEdit={setRuleEdit}
          busy={busy}
          guard={guard}
        />
      )}
      {tab === "cats" && (
        <StaffCatsPanel
          packs={packs}
          cats={cats}
          applies={applies}
          curSiteType={curSiteType}
          packMode={packMode}
          setPackMode={setPackMode}
          catName={catName}
          catParent={catParent}
          setCatParent={setCatParent}
          setCatName={setCatName}
          busy={busy}
          guard={guard}
        />
      )}
    </>
  );
}
