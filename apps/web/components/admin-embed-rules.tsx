"use client";

/**
 * 视频内嵌规则管理（0189）：/admin?tool=embedrules。
 * video_embed_rules 表的列表 + 停用/删除；新建/编辑抽屉在
 * admin-embed-rule-editor.tsx。内置规则（builtin）只许停用不许删。
 */
import { useCallback, useEffect, useState } from "react";
import { api, ApiError } from "@/lib/api-client";
import { useI18n } from "@/i18n/client";
import { EmbedRuleEditor } from "@/components/admin-embed-rule-editor";

export interface EmbedRuleAdmin {
  id: number;
  provider: string;
  name_zh: string;
  url_pattern: string;
  embed_template: string;
  embed_origin: string;
  render_kind: "iframe" | "video";
  aspect: "16:9" | "4:3" | "1:1";
  extra_params: string | null;
  builtin: boolean;
  enabled: boolean;
  sort: number;
}

const BTN =
  "rounded-full border border-line px-3 py-1 text-xs font-bold text-sub " +
  "hover:border-sky hover:text-sky";

export function AdminEmbedRules() {
  const { dict } = useI18n();
  const t = dict.adminEmbedRules;
  const [rows, setRows] = useState<EmbedRuleAdmin[] | null>(null);
  const [editing, setEditing] = useState<Partial<EmbedRuleAdmin> | null>(null);
  const [msg, setMsg] = useState<string | null>(null);

  const load = useCallback(async () => {
    try {
      setRows(await api.get<EmbedRuleAdmin[]>("/api/v1/admin/embed-rules"));
    } catch {
      setRows([]);
      setMsg(t.loadFailed);
    }
  }, [t.loadFailed]);

  useEffect(() => {
    load();
  }, [load]);

  async function toggle(r: EmbedRuleAdmin) {
    try {
      await api.put(`/api/v1/admin/embed-rules/${r.id}`, {
        provider: r.provider,
        name_zh: r.name_zh,
        url_pattern: r.url_pattern,
        embed_template: r.embed_template,
        embed_origin: r.embed_origin,
        render_kind: r.render_kind,
        aspect: r.aspect,
        extra_params: r.extra_params,
        enabled: !r.enabled,
        sort: r.sort,
      });
      load();
    } catch {
      setMsg(dict.common.networkError);
    }
  }

  async function del(r: EmbedRuleAdmin) {
    if (!window.confirm(t.delConfirm.replace("{name}", r.name_zh))) return;
    try {
      await api.post(`/api/v1/admin/embed-rules/${r.id}/delete`);
      load();
    } catch (e) {
      setMsg(
        e instanceof ApiError && e.message
          ? e.message
          : dict.common.networkError,
      );
    }
  }

  return (
    <section className="nexus-detail">
      <h2 className="mb-1 text-base font-bold text-ink">{t.title}</h2>
      <p className="mb-3 text-xs text-sub">{t.hint}</p>
      {msg && (
        <p
          className="mb-3 rounded-[var(--r-md)] bg-sky-soft p-3 text-sm"
          role="alert"
        >
          {msg}
        </p>
      )}
      <button
        type="button"
        onClick={() =>
          setEditing({
            render_kind: "iframe",
            aspect: "16:9",
            enabled: true,
            sort: 100,
          })
        }
        className={`${BTN} mb-3 self-start`}
      >
        + {t.newBtn}
      </button>
      {editing && (
        <EmbedRuleEditor
          editing={editing}
          setEditing={setEditing}
          onSaved={(m) => {
            setMsg(m);
            load();
          }}
        />
      )}
      <div className="overflow-x-auto">
        <table className="nexus-table">
          <thead>
            <tr>
              <th>{t.colName}</th>
              <th>{t.colPattern}</th>
              <th>{t.colKind}</th>
              <th>{t.colState}</th>
              <th>{t.colOps}</th>
            </tr>
          </thead>
          <tbody>
            {(rows ?? []).map((r) => (
              <tr key={r.id}>
                <td className="font-bold">
                  {r.name_zh}
                  {r.builtin && (
                    <span className="ml-1 text-[10px] text-sub">
                      ({t.builtin})
                    </span>
                  )}
                </td>
                <td className="max-w-[280px] truncate font-mono text-[11px]">
                  {r.url_pattern}
                </td>
                <td>{r.render_kind}</td>
                <td>{r.enabled ? t.on : t.off}</td>
                <td>
                  <div className="flex gap-2">
                    <button
                      type="button"
                      onClick={() => setEditing(r)}
                      className={BTN}
                    >
                      {t.edit}
                    </button>
                    <button
                      type="button"
                      onClick={() => toggle(r)}
                      className={BTN}
                    >
                      {r.enabled ? t.disable : t.enable}
                    </button>
                    {!r.builtin && (
                      <button
                        type="button"
                        onClick={() => del(r)}
                        className={BTN}
                      >
                        {t.del}
                      </button>
                    )}
                  </div>
                </td>
              </tr>
            ))}
            {rows && rows.length === 0 && (
              <tr>
                <td colSpan={5} className="py-6 text-center text-sub">
                  {t.empty}
                </td>
              </tr>
            )}
          </tbody>
        </table>
      </div>
    </section>
  );
}
