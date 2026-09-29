"use client";

/**
 * 新建奖品（后台写侧）。
 *
 * 从 admin-arcade-items 拆出：目录表负责「改已有」，这里负责「造新的」——
 * 两件事的校验面不同（新建要管 key 唯一与初值，编辑要管跨池 EV 回查），
 * 叠在一个文件里既看不清也会撞行数上限。
 *
 * 走的是同一个 upsert 端点：站长给一件新虚拟奖品定价是正常动作，
 * 所以 anchor_src 记 `declared`（运营自估），服务端保存时仍会按新值
 * 跨池回查 EV，不合法整体回滚 —— 可配不等于没闸。
 */

import { useState } from "react";
import { useI18n } from "@/i18n/client";
import { api, ApiError } from "@/lib/api-client";

interface Draft {
  key: string;
  name: string;
  kind: string;
  anchor: number;
  icon: string;
  per_user: number;
  stock: number;
  unlimited: boolean;
}

const EMPTY: Draft = {
  key: "",
  name: "",
  kind: "cosmetic",
  anchor: 0,
  icon: "🎁",
  per_user: 5,
  stock: 0,
  unlimited: true,
};

const CELL =
  "w-full rounded-[var(--r-sm)] border border-line " +
  "bg-[var(--surface-card)] px-1.5 py-1 text-xs";

const BAR =
  "flex flex-wrap items-end gap-2 rounded-[var(--r-sm)] " +
  "border border-line p-2";

const BTN =
  "rounded-full border border-line px-3 py-1 text-[11px] " +
  "font-bold disabled:opacity-50";

export function AdminArcadeItemsNew({
  onCreated,
  busy,
}: {
  onCreated: () => void;
  busy: boolean;
}) {
  const { dict } = useI18n();
  const t = dict.adminArcade.items;
  const [draft, setDraft] = useState<Draft>(EMPTY);
  const [msg, setMsg] = useState<string | null>(null);
  const [busySelf, setBusySelf] = useState(false);

  async function create() {
    setBusySelf(true);
    setMsg(null);
    try {
      await api.post("/api/v1/admin/arcade/items", {
        ...draft,
        anchor_src: draft.anchor > 0 ? "declared" : "n/a",
        enabled: true,
      });
      setMsg(t.created.replace("{k}", draft.key));
      setDraft({ ...EMPTY });
      onCreated();
    } catch (err) {
      setMsg(err instanceof ApiError ? err.message : t.saveFail);
    }
    setBusySelf(false);
  }

  return (
    <div className="flex flex-col gap-1">
      <div className={BAR}>
        <span className="text-[11px] font-bold text-sub">{t.newTitle}</span>
        <input
          className={CELL + " w-28"}
          placeholder="key"
          value={draft.key}
          onChange={(e) => setDraft({ ...draft, key: e.target.value })}
        />
        <input
          className={CELL + " w-36"}
          placeholder={t.colName}
          value={draft.name}
          onChange={(e) => setDraft({ ...draft, name: e.target.value })}
        />
        <input
          className={CELL + " w-24"}
          placeholder={t.colKind}
          value={draft.kind}
          onChange={(e) => setDraft({ ...draft, kind: e.target.value })}
        />
        <input
          className={CELL + " w-14"}
          placeholder={t.colIcon}
          value={draft.icon}
          onChange={(e) => setDraft({ ...draft, icon: e.target.value })}
        />
        <input
          type="number"
          className={CELL + " w-24 text-right"}
          placeholder={t.colAnchor}
          value={draft.anchor}
          onChange={(e) =>
            setDraft({ ...draft, anchor: Number(e.target.value) || 0 })
          }
        />
        <input
          type="number"
          className={CELL + " w-20 text-right"}
          placeholder={t.colPerUser}
          value={draft.per_user}
          onChange={(e) =>
            setDraft({ ...draft, per_user: Number(e.target.value) || 1 })
          }
        />
        <label className="flex items-center gap-1 text-[11px]">
          <input
            type="checkbox"
            checked={draft.unlimited}
            onChange={(e) =>
              setDraft({ ...draft, unlimited: e.target.checked })
            }
          />
          {t.colUnlimited}
        </label>
        <button
          type="button"
          className={BTN}
          disabled={busy || busySelf || !draft.key.trim()}
          onClick={() => void create()}
        >
          {busySelf ? t.saving : t.create}
        </button>
      </div>
      {msg && <p className="text-[11px] text-sub">{msg}</p>}
    </div>
  );
}
