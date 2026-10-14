"use client";

import { useState } from "react";
import { api } from "@/lib/api-client";
import { useI18n } from "@/i18n/client";
// 接口类型自 OpenAPI 注解生成（FRAME §7 第 8 条分域起步）：
// scripts/gen_ts_types.mjs ← /api/v1/openapi.json ← #[utoipa::path]
import type {
  RssTokenCreateReq,
  RssTokenCreated,
  RssTokenList,
  RssTokenMeta,
} from "@fluxtorrent/domain-types";

export type { RssTokenMeta };

/** 暂存明文的行 = 元数据 + 本次会话暂存的明文（刷新后不可再取） */
interface TokenRow extends RssTokenMeta {
  plain?: string;
}

function buildUrl(base: string, cred: string): string {
  return `${base}${cred}`;
}

/** 复制按钮（三个动作共用）：点击后短暂显示「已复制」 */
function CopyBtn({
  text,
  label,
  copiedLabel,
}: {
  text: string;
  label: string;
  copiedLabel: string;
}) {
  const [done, setDone] = useState(false);
  return (
    <button
      type="button"
      className="rss-copy"
      onClick={async () => {
        try {
          await navigator.clipboard.writeText(text);
          setDone(true);
          setTimeout(() => setDone(false), 1500);
        } catch {
          /* 剪贴板权限被拒：忽略，用户可手动选中复制 */
        }
      }}
    >
      {done ? copiedLabel : label}
    </button>
  );
}

/** token 管理面板：列表（复制/轮换/撤销）+ 新建表单。
 *  明文生命周期：仅创建/轮换响应当场暂存在本组件状态里，刷新即失。 */
export function RssTokenPanel({
  base,
  initial,
}: {
  base: string;
  initial: RssTokenMeta[];
}) {
  const { dict } = useI18n();
  const t = dict.getrss;
  const [rows, setRows] = useState<TokenRow[]>(
    initial.map((m) => ({ ...m })),
  );
  const [name, setName] = useState("");
  const [busy, setBusy] = useState(false);
  const [err, setErr] = useState("");

  async function refresh() {
    const d = await api.get<RssTokenList>("/api/v1/rss/tokens");
    // 保留本地暂存的明文（列表刷新不丢「本次会话可复制」状态）
    setRows((old) =>
      d.tokens.map((m) => ({
        ...m,
        plain: old.find((o) => o.id === m.id)?.plain,
      })),
    );
  }

  async function create() {
    setBusy(true);
    setErr("");
    try {
      const body: RssTokenCreateReq = { name: name.trim() };
      const r = await api.post<RssTokenCreated>("/api/v1/rss/tokens", body);
      setRows((old) => [{ ...r }, ...old]);
      setName("");
    } catch (e) {
      setErr(e instanceof Error ? e.message : String(e));
    } finally {
      setBusy(false);
    }
  }

  async function rotate(id: number) {
    setBusy(true);
    setErr("");
    try {
      const r = await api.post<RssTokenCreated>(
        `/api/v1/rss/tokens/${id}/rotate`,
      );
      await refresh();
      setRows((old) =>
        old.map((m) => (m.id === id ? { ...r } : m)),
      );
    } catch (e) {
      setErr(e instanceof Error ? e.message : String(e));
    } finally {
      setBusy(false);
    }
  }

  async function revoke(id: number) {
    setBusy(true);
    setErr("");
    try {
      await api.del(`/api/v1/rss/tokens/${id}`);
      setRows((old) => old.filter((m) => m.id !== id));
    } catch (e) {
      setErr(e instanceof Error ? e.message : String(e));
    } finally {
      setBusy(false);
    }
  }

  return (
    <section className="rss-card">
      <h2 className="rss-card__title">{t.tokenTitle}</h2>
      <p className="rss-hint">{t.tokenNote}</p>
      <div className="rss-rows">
        <input
          type="text"
          value={name}
          maxLength={40}
          onChange={(e) => setName(e.target.value)}
          placeholder={t.tokenNamePh}
        />
        <button type="button" disabled={busy} onClick={create}>
          {t.tokenCreate}
        </button>
      </div>
      {err && <p className="rss-hint">{err}</p>}
      {rows.length === 0 ? (
        <p className="rss-hint">{t.tokenEmpty}</p>
      ) : (
        <ul className="rss-presets">
          {rows.map((m) => (
            <li key={m.id} className="rss-preset">
              <span>{m.name}</span>
              {m.plain ? (
                <input
                  type="text"
                  readOnly
                  value={buildUrl(base, m.plain)}
                  onFocus={(e) => e.target.select()}
                />
              ) : (
                <small className="rss-hint">{t.tokenPlainGone}</small>
              )}
              <small className="rss-hint">
                {new Date(m.created_at).toLocaleString()}
              </small>
              {m.plain && (
                <CopyBtn
                  text={buildUrl(base, m.plain)}
                  label={t.copy}
                  copiedLabel={t.copied}
                />
              )}
              <button
                type="button"
                disabled={busy}
                onClick={() => rotate(m.id)}
              >
                {t.tokenRotate}
              </button>
              <button
                type="button"
                disabled={busy}
                onClick={() => revoke(m.id)}
              >
                {t.tokenRevoke}
              </button>
            </li>
          ))}
        </ul>
      )}
    </section>
  );
}
