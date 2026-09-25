"use client";

/**
 * 视频内嵌规则管理（0189，从 admin-embed-rules.tsx 拆出，300 门禁）：
 * 新建/编辑抽屉表单。内置规则（builtin）只许停用不许删；
 * 正则/模板校验在后端保存时把关（保存被拒时后端消息透传展示）。
 */
import { api, ApiError } from "@/lib/api-client";
import { useI18n } from "@/i18n/client";
import type { EmbedRuleAdmin } from "@/components/admin-embed-rules";

const INPUT =
  "min-h-[38px] rounded-[var(--r-sm)] border border-line bg-cloud px-3 " +
  "text-sm outline-none focus:border-sky";
const BTN =
  "rounded-full border border-line px-3 py-1 text-xs font-bold text-sub " +
  "hover:border-sky hover:text-sky";
const SAVE_BTN =
  "rounded-full bg-sky px-4 py-1.5 text-xs font-bold text-white";

export function EmbedRuleEditor({
  editing,
  setEditing,
  onSaved,
}: {
  editing: Partial<EmbedRuleAdmin>;
  setEditing: (r: Partial<EmbedRuleAdmin> | null) => void;
  onSaved: (msg: string) => void;
}) {
  const { dict } = useI18n();
  const t = dict.adminEmbedRules;
  const e = editing;

  async function save() {
    const body = {
      provider: e.provider ?? "",
      name_zh: e.name_zh ?? "",
      url_pattern: e.url_pattern ?? "",
      embed_template: e.embed_template ?? "",
      embed_origin: e.embed_origin ?? "",
      render_kind: e.render_kind ?? "iframe",
      aspect: e.aspect ?? "16:9",
      extra_params: e.extra_params || null,
      enabled: e.enabled ?? true,
      sort: e.sort ?? 100,
    };
    try {
      if (e.id) {
        await api.put(`/api/v1/admin/embed-rules/${e.id}`, body);
      } else {
        await api.post("/api/v1/admin/embed-rules", body);
      }
      setEditing(null);
      onSaved(t.saved);
    } catch (err) {
      onSaved(
        err instanceof ApiError && err.message
          ? err.message
          : dict.common.networkError,
      );
    }
  }

  return (
    <div className={EDIT_BOX_PARENT}>
      <div className="grid gap-2 sm:grid-cols-2">
        <input
          value={e.provider ?? ""}
          onChange={(x) => setEditing({ ...e, provider: x.target.value })}
          placeholder={t.fProvider}
          className={INPUT}
        />
        <input
          value={e.name_zh ?? ""}
          onChange={(x) => setEditing({ ...e, name_zh: x.target.value })}
          placeholder={t.fName}
          className={INPUT}
        />
      </div>
      <input
        value={e.url_pattern ?? ""}
        onChange={(x) => setEditing({ ...e, url_pattern: x.target.value })}
        placeholder={t.fPattern}
        className={`${INPUT} font-mono text-xs`}
      />
      <input
        value={e.embed_template ?? ""}
        onChange={(x) => setEditing({ ...e, embed_template: x.target.value })}
        placeholder={t.fTemplate}
        className={`${INPUT} font-mono text-xs`}
      />
      <input
        value={e.embed_origin ?? ""}
        onChange={(x) => setEditing({ ...e, embed_origin: x.target.value })}
        placeholder={t.fOrigin}
        className={`${INPUT} font-mono text-xs`}
      />
      <div className="grid gap-2 sm:grid-cols-3">
        <select
          value={e.render_kind ?? "iframe"}
          onChange={(x) =>
            setEditing({
              ...e,
              render_kind: x.target.value as "iframe" | "video",
            })
          }
          className={INPUT}
        >
          <option value="iframe">iframe</option>
          <option value="video">video</option>
        </select>
        <select
          value={e.aspect ?? "16:9"}
          onChange={(x) =>
            setEditing({
              ...e,
              aspect: x.target.value as "16:9" | "4:3" | "1:1",
            })
          }
          className={INPUT}
        >
          <option value="16:9">16:9</option>
          <option value="4:3">4:3</option>
          <option value="1:1">1:1</option>
        </select>
        <input
          type="number"
          value={e.sort ?? 100}
          onChange={(x) => setEditing({ ...e, sort: Number(x.target.value) })}
          placeholder={t.fSort}
          className={INPUT}
        />
      </div>
      <div className="flex gap-2">
        <button type="button" onClick={save} className={SAVE_BTN}>
          {t.save}
        </button>
        <button type="button" onClick={() => setEditing(null)} className={BTN}>
          {dict.common.cancel}
        </button>
      </div>
    </div>
  );
}

const EDIT_BOX_PARENT =
  "mb-4 flex flex-col gap-2 rounded-[var(--r-md)] border border-line " +
  "bg-[var(--surface-card)] p-3";
