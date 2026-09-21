"use client";

import { useCallback, useEffect, useState } from "react";
import { api, ApiError } from "@/lib/api-client";
import { useI18n } from "@/i18n/client";
import type { ToolTab } from "@/components/staff-tools";

/** 内容域面板（从 staff-tools.tsx 按域拆出，300 行门禁）：
 *  FAQ 管理（faq）/ 规则管理（rules）/ 分类管理（cats，含类型包切换）。 */

interface FaqItem {
  id: number;
  category: string;
  question: string;
  answer: string;
  sort: number;
}
interface RuleItem {
  id: number;
  title: string;
  body: string;
  sort: number;
}
interface CatItem {
  id: number;
  name: string;
  torrents: number;
}
interface TypePack {
  code: string;
  name: string;
  description: string | null;
  brand: string;
  categories: { id: number; name: string }[];
  modules: Record<string, boolean>;
  sort: number;
}

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
                    className="min-h-[36px] rounded-full border border-line px-4 text-xs font-bold"
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

      {/* 规则管理 */}
      {tab === "rules" && (
        <>
          <section className="baozi-panel p-4">
            <h2 className="mb-3 text-base font-bold text-ink">
              {ruleEdit.id === null ? t.ruleNew : t.ruleEdit}
            </h2>
            <div className="cmgmt-form">
              <label>
                {dict.cmgmt.fldTitle}
                <input
                  value={ruleEdit.title}
                  onChange={(e) =>
                    setRuleEdit({ ...ruleEdit, title: e.target.value })
                  }
                />
              </label>
              <label>
                {dict.cmgmt.fldBody}
                <textarea
                  rows={5}
                  value={ruleEdit.body}
                  onChange={(e) =>
                    setRuleEdit({ ...ruleEdit, body: e.target.value })
                  }
                />
              </label>
              <div className="flex gap-2">
                <button
                  className="baozi-button"
                  disabled={busy || !ruleEdit.title.trim()}
                  onClick={() =>
                    guard(async () => {
                      if (ruleEdit.id === null)
                        await api.post("/api/v1/admin/rules", {
                          title: ruleEdit.title,
                          body: ruleEdit.body,
                        });
                      else
                        await api.put(`/api/v1/admin/rules/${ruleEdit.id}`, {
                          title: ruleEdit.title,
                          body: ruleEdit.body,
                        });
                      setRuleEdit({ id: null, title: "", body: "" });
                    }, t.saved)
                  }
                >
                  {t.btnSave}
                </button>
                {ruleEdit.id !== null && (
                  <button
                    className="min-h-[36px] rounded-full border border-line px-4 text-xs font-bold"
                    onClick={() =>
                      setRuleEdit({ id: null, title: "", body: "" })
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
                <td className="colhead">{dict.cmgmt.fldTitle}</td>
                <td className="colhead text-right">{dict.cmgmt.colActions}</td>
              </tr>
              {rules.map((r) => (
                <tr key={r.id}>
                  <td>{r.title}</td>
                  <td className="text-right">
                    <button
                      className="cmgmt-act"
                      onClick={() =>
                        setRuleEdit({ id: r.id, title: r.title, body: r.body })
                      }
                    >
                      {dict.cmgmt.btnEdit}
                    </button>
                    <button
                      className="cmgmt-act cmgmt-act--danger"
                      onClick={() =>
                        guard(async () => {
                          await api.del(`/api/v1/admin/rules/${r.id}`);
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

      {/* 分类管理 */}
      {tab === "cats" && (
        <>
          <section className="baozi-panel p-4">
            <h2 className="mb-3 text-base font-bold text-ink">{t.packTitle}</h2>
            <p className="mb-3 text-xs text-sub">{t.packNote}</p>
            <div className="mb-2 flex items-center gap-3">
              <span className="text-xs font-bold text-sub">{t.packMode}</span>
              <label className="flex items-center gap-1 text-xs">
                <input
                  type="radio"
                  checked={packMode === "replace"}
                  onChange={() => setPackMode("replace")}
                />
                {t.packModeReplace}
              </label>
              <label className="flex items-center gap-1 text-xs">
                <input
                  type="radio"
                  checked={packMode === "merge"}
                  onChange={() => setPackMode("merge")}
                />
                {t.packModeMerge}
              </label>
            </div>
            <div className="flex flex-wrap gap-2">
              {packs.map((pk) => {
                const active = pk.code === curSiteType;
                return (
                  <span
                    key={pk.code}
                    className="inline-flex items-center gap-1"
                  >
                    <button
                      className={`min-h-[40px] rounded-full border px-4 text-xs font-bold disabled:opacity-50 ${
                        active
                          ? "border-[var(--baozi-orange)] bg-[var(--baozi-orange)] text-white"
                          : "border-[var(--baozi-orange)] text-[var(--baozi-orange-dark)]"
                      }`}
                      disabled={busy}
                      title={pk.description ?? ""}
                      onClick={() => {
                        // U2 §8.2 切换向导：先 diff 预览（旧值→新值），确认后才 apply
                        void (async () => {
                          let lines: string[] = [];
                          try {
                            const d = await api.post<{
                              changes: {
                                key: string;
                                old: string;
                                new: string;
                              }[];
                            }>("/api/v1/admin/site-type-packs/diff", {
                              code: pk.code,
                            });
                            lines = d.changes.map(
                              (c) => `${c.key}: ${c.old} → ${c.new}`,
                            );
                          } catch {
                            /* diff 失败不阻塞——回落旧确认文案 */
                          }
                          const detail = lines.length
                            ? `将变更 ${lines.length} 项：\n${lines.slice(0, 15).join("\n")}${lines.length > 15 ? "\n…" : ""}`
                            : "无配置差异（分类重建仍会执行）";
                          if (
                            !window.confirm(
                              `${t.packConfirm.replace("{name}", pk.name)}\n\n${detail}`,
                            )
                          )
                            return;
                          await guard(
                            async () => {
                              await api.post(
                                "/api/v1/admin/site-type-packs/apply",
                                { code: pk.code, mode: packMode },
                              );
                            },
                            t.packApplied.replace("{name}", pk.name),
                          );
                        })();
                      }}
                    >
                      {pk.name}
                      {active ? `（${t.packCurrent}）` : ""}
                    </button>
                    {/* 自定义站型入口（U5 分发）：另存当前配置为新包 */}
                    <button
                      className="rounded-full border border-line px-3 text-xs text-sub disabled:opacity-50"
                      disabled={busy}
                      title={t.packSaveTip}
                      onClick={() => {
                        const name = window.prompt(t.packSavePrompt);
                        if (!name?.trim()) return;
                        void guard(async () => {
                          await api.post("/api/v1/admin/site-type-packs/save", {
                            code: `custom_${name
                              .trim()
                              .toLowerCase()
                              .replace(/[^a-z0-9_]+/g, "_")
                              .slice(0, 32)}`,
                            name: name.trim(),
                          });
                        }, t.packSaved);
                      }}
                    >
                      💾
                    </button>
                  </span>
                );
              })}
            </div>
          </section>
          <section className="baozi-panel p-4">
            <h2 className="mb-3 text-base font-bold text-ink">{t.tabCats}</h2>
            <div className="cmgmt-form">
              <label>
                {t.fldCatName}
                <input
                  value={catName}
                  onChange={(e) => setCatName(e.target.value)}
                />
              </label>
              <button
                className="baozi-button self-start"
                disabled={busy || !catName.trim()}
                onClick={() =>
                  guard(async () => {
                    await api.post("/api/v1/admin/categories", {
                      name: catName,
                    });
                    setCatName("");
                  }, t.saved)
                }
              >
                {t.btnAddCat}
              </button>
            </div>
            <table className="nexus-table mt-3">
              <tbody>
                <tr>
                  <td className="colhead">#</td>
                  <td className="colhead">{t.fldCatName}</td>
                  <td className="colhead">{t.catTorrents}</td>
                  <td className="colhead text-right">
                    {dict.cmgmt.colActions}
                  </td>
                </tr>
                {cats.map((c) => (
                  <tr key={c.id}>
                    <td className="num">{c.id}</td>
                    <td>{c.name}</td>
                    <td className="num">{c.torrents}</td>
                    <td className="text-right">
                      <button
                        className="cmgmt-act"
                        onClick={() => {
                          const nn = prompt(t.renamePrompt, c.name);
                          if (nn && nn !== c.name)
                            void guard(async () => {
                              await api.put(
                                `/api/v1/admin/categories/${c.id}`,
                                { name: nn },
                              );
                            }, t.saved);
                        }}
                      >
                        {dict.cmgmt.btnEdit}
                      </button>
                      <button
                        className="cmgmt-act cmgmt-act--danger"
                        onClick={() =>
                          guard(async () => {
                            await api.del(`/api/v1/admin/categories/${c.id}`);
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
          </section>
        </>
      )}
    </>
  );
}
