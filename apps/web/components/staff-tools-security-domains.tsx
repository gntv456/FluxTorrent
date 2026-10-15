"use client";

/**
 * 安全域·一次性邮箱域名库子面板（0370 M4 注册治理第 ④ 道闸）：
 * 独立工具页 `?tool=emaildomains`。列表（q 模糊）/ 批量导入（粘贴或
 * JSON 数组，可含整邮箱与 URL，后端归一化）/ 导出 JSON / 按域名删除。
 *
 * 与「邮件黑白名单」同族但语义不同——黑白名单是**规则**（pattern + 模式），
 * 本库是**清单**（域名全集），命中判定在 auth_http/register_gates.rs
 * （含子域后缀匹配）。动作用本地 busy + 父级 flash 上抛。
 */

import { useCallback, useEffect, useState } from "react";
import { api, ApiError } from "@/lib/api-client";
import { useI18n } from "@/i18n/client";
import { dateLocale, fmt } from "@/i18n/config";
import type { EmailDomainRow } from "./staff-tools-security-shared";

interface ListResp {
  items: EmailDomainRow[];
  total: number;
}

interface ImportResp {
  added: number;
  skipped: number;
  invalid: number;
}

export function StaffEmailDomainsPanel({
  flash,
}: {
  flash: (m: string) => void;
}) {
  const { dict, locale } = useI18n();
  const t = dict.stafftools;
  const [rows, setRows] = useState<EmailDomainRow[] | null>(null);
  const [total, setTotal] = useState(0);
  const [q, setQ] = useState("");
  const [needle, setNeedle] = useState("");
  const [text, setText] = useState("");
  const [busy, setBusy] = useState(false);

  const load = useCallback(async () => {
    setRows(null);
    const qs = needle
      ? `?limit=500&q=${encodeURIComponent(needle)}`
      : "?limit=500";
    try {
      const r = await api.get<ListResp>(`/api/v1/admin/email-domains${qs}`);
      setRows(r.items);
      setTotal(r.total);
    } catch {
      setRows([]);
    }
  }, [needle]);

  useEffect(() => {
    void load();
  }, [load]);

  async function run(fn: () => Promise<void>, ok?: string) {
    setBusy(true);
    try {
      await fn();
      if (ok) flash(ok);
      await load();
    } catch (e) {
      flash(e instanceof ApiError ? e.message : dict.common.networkError);
    } finally {
      setBusy(false);
    }
  }

  function doImport() {
    const raw = text.trim();
    if (!raw) return;
    // 单行 JSON 数组按数组传；否则按换行/逗号/分号的纯文本传
    let payload: { domains: string[] } | { text: string };
    if (raw.startsWith("[")) {
      let domains: string[];
      try {
        const parsed = JSON.parse(raw) as unknown;
        if (!Array.isArray(parsed)) throw new Error("not-array");
        domains = parsed.map((x) => String(x));
      } catch {
        flash(t.edBadJson);
        return;
      }
      payload = { domains };
    } else {
      payload = { text: raw };
    }
    void run(async () => {
      const r = await api.post<ImportResp>(
        "/api/v1/admin/email-domains/import",
        payload,
      );
      setText("");
      flash(
        fmt(t.edImported, {
          added: r.added,
          skipped: r.skipped,
          invalid: r.invalid,
        }),
      );
    });
  }

  function doExport() {
    void (async () => {
      setBusy(true);
      try {
        const r = await api.get<{ domains: string[]; count: number }>(
          "/api/v1/admin/email-domains/export",
        );
        const blob = new Blob([JSON.stringify(r.domains, null, 2)], {
          type: "application/json",
        });
        const url = URL.createObjectURL(blob);
        const a = document.createElement("a");
        a.href = url;
        a.download = "disposable-email-domains.json";
        a.click();
        URL.revokeObjectURL(url);
        flash(fmt(t.edExported, { n: r.count }));
      } catch (e) {
        flash(e instanceof ApiError ? e.message : dict.common.networkError);
      } finally {
        setBusy(false);
      }
    })();
  }

  return (
    <>
      <section className="baozi-panel p-4">
        <h2 className="mb-1 text-base font-bold text-ink">{t.edImport}</h2>
        <p className="mb-3 text-xs text-sub">{t.edNote}</p>
        <div className="cmgmt-form">
          <label>
            {t.edImportLabel}
            <textarea
              rows={6}
              value={text}
              onChange={(e) => setText(e.target.value)}
              placeholder={t.edImportPh}
            />
          </label>
          <div className="flex flex-wrap gap-2 self-start">
            <button
              className="baozi-button"
              disabled={busy || !text.trim()}
              onClick={doImport}
            >
              {t.edImportBtn}
            </button>
            <button
              className="baozi-button"
              disabled={busy}
              onClick={doExport}
            >
              {t.edExportBtn}
            </button>
          </div>
        </div>
      </section>

      <section className="baozi-panel mt-4 p-4">
        <div className="mb-3 flex flex-wrap items-end gap-2">
          <label className="flex flex-col gap-1">
            <span className="text-xs text-sub">{t.edSearch}</span>
            <input
              value={q}
              onChange={(e) => setQ(e.target.value)}
              placeholder={t.edSearchPh}
              onKeyDown={(e) => {
                if (e.key === "Enter") setNeedle(q.trim());
              }}
            />
          </label>
          <button
            className="baozi-button"
            disabled={busy}
            onClick={() => setNeedle(q.trim())}
          >
            {t.edSearchBtn}
          </button>
          {needle && (
            <button
              className="baozi-button"
              disabled={busy}
              onClick={() => {
                setQ("");
                setNeedle("");
              }}
            >
              {t.edClear}
            </button>
          )}
          <span className="pb-2 text-xs text-sub">
            {fmt(t.edTotal, { n: total })}
          </span>
        </div>

        <table className="nexus-table">
          <tbody>
            <tr>
              <td className="colhead">{t.edColDomain}</td>
              <td className="colhead">{t.edColSource}</td>
              <td className="colhead">{t.edColTime}</td>
              <td className="colhead text-right">
                {dict.cmgmt.colActions}
              </td>
            </tr>
            {(rows ?? []).map((r) => (
              <tr key={r.domain}>
                <td className="font-mono">{r.domain}</td>
                <td>
                  {r.source === "builtin"
                    ? t.edSourceBuiltin
                    : t.edSourceImport}
                </td>
                <td className="text-xs text-sub">
                  {new Date(r.created_at).toLocaleString(dateLocale(locale))}
                </td>
                <td className="text-right">
                  <button
                    className="cmgmt-act cmgmt-act--danger"
                    onClick={() =>
                      void run(async () => {
                        await api.post("/api/v1/admin/email-domains/delete", {
                          domains: [r.domain],
                        });
                      }, t.edDeleted)
                    }
                  >
                    {dict.cmgmt.btnDelete}
                  </button>
                </td>
              </tr>
            ))}
            {rows !== null && rows.length === 0 && (
              <tr>
                <td colSpan={4} className="py-6 text-center text-sub">
                  {t.edEmpty}
                </td>
              </tr>
            )}
            {rows === null && (
              <tr>
                <td colSpan={4} className="py-6 text-center text-sub">
                  …
                </td>
              </tr>
            )}
          </tbody>
        </table>
        {rows !== null && rows.length >= 500 && (
          <p className="mt-2 text-xs text-sub">{t.edTruncated}</p>
        )}
      </section>
    </>
  );
}
