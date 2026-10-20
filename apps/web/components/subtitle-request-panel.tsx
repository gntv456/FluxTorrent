"use client";

import { useCallback, useEffect, useState } from "react";
import { api, ApiError } from "@/lib/api-client";
import { apiErrorMessage, useI18n } from "@/i18n/client";
import type { SubtitleRequestRow } from "@/components/subtitle-board-shared";
import { SubtitleRequestActions } from "@/components/subtitle-request-actions";
import type { SubReqAct } from "@/components/subtitle-request-actions";
import { SubtitleRequestCards } from "@/components/subtitle-request-cards";
import { useIsCompact } from "@/lib/hooks/use-media";
import { ConfirmDialog } from "@/components/ui/confirm-dialog";
import { toast } from "@/components/ui/toast";

/** 求字幕悬赏 + 工作流（0146 pots → 0148 认领/交稿/验收/协作/free）。
 *  从 subtitle-board.tsx 按域拆出（300 行门禁）；附件上传共用件在尾部。 */

type LangItem = { code: string; name: string; flag: string };

export function SubtitleRequestPanel({
  langs,
  fixedTorrentId,
  onMsg,
  reloadKey,
}: {
  langs: LangItem[];
  fixedTorrentId?: number;
  onMsg: (m: string) => void;
  reloadKey: unknown;
}) {
  const { dict, currency } = useI18n();
  const t = dict.subtitles;
  const [reqs, setReqs] = useState<SubtitleRequestRow[]>([]);
  const [rLang, setRLang] = useState("");
  const [rBounty, setRBounty] = useState("");
  const [rFree, setRFree] = useState(false);
  const [busy, setBusy] = useState(false);
  const [me, setMe] = useState<{ id: number; username?: string } | null>(
    null,
  );
  // 工作流开关（site-profile 暴露；关 = NP 直交付口径）
  const [wfOn, setWfOn] = useState(false);
  // A1 反馈协议：prompt/confirm 弹窗 → ConfirmDialog + 输入槽；结果走 toast
  const [ask, setAsk] = useState<{
    kind: SubReqAct | "contribute";
    row: SubtitleRequestRow;
  } | null>(null);
  const [input, setInput] = useState("");

  const loadReqs = useCallback(async () => {
    try {
      setReqs(
        await api.get<SubtitleRequestRow[]>(
          "/api/v1/subtitles/requests?status=open",
        ),
      );
    } catch {
      setReqs([]); toast.error(dict.common.loadFailed);
    }
  }, [dict]);

  useEffect(() => {
    loadReqs();
    api
      .get<{ id: number; username?: string }>("/api/v1/me")
      .then(setMe)
      .catch(() => setMe(null));
    fetch("/api/v1/site-profile")
      .then((r) => r.json())
      .then((b: { data?: { subtitle_workflow?: boolean } }) =>
        setWfOn(!!b.data?.subtitle_workflow),
      )
      .catch(() => {});
  }, [loadReqs, reloadKey]);
  // M5.6 §13.6：窄屏（<640）悬赏表降级为卡片列表（见 subtitle-request-cards）
  const compact = useIsCompact();

  async function createReq() {
    if (!rLang) return;
    setBusy(true);
    try {
      await api.post("/api/v1/subtitles/requests", {
        lang: rLang,
        torrent_id: fixedTorrentId ?? undefined,
        bounty: rBounty ? Number(rBounty) : 0,
        offer_free: rFree,
      });
      setRLang("");
      setRBounty("");
      setRFree(false);
      toast.success(t.reqOk?.replace("{magic}", currency) ?? "created");
      loadReqs();
    } catch (e) {
      toast.error(String((e as Error)?.message ?? dict.common.networkError), {
        requestId: e instanceof ApiError ? e.requestId : undefined,
      });
    } finally {
      setBusy(false);
    }
  }

  function openAsk(kind: SubReqAct | "contribute", row: SubtitleRequestRow) {
    setInput("");
    setAsk({ kind, row });
  }

  /** 确认对话框提交：按 kind 补输入校验再发请求 */
  async function confirmAsk() {
    if (!ask) return;
    const { kind, row } = ask;
    try {
      if (kind === "contribute") {
        const amount = Number(input.replace(/\D/g, "")) || 0;
        await api.post(`/api/v1/subtitles/requests/${row.id}/contribute`, {
          amount,
        });
      } else if (kind === "claim") {
        const crew = input
          .split(/[,，]/)
          .map((s) => s.trim())
          .filter(Boolean)
          .map((s) => {
            const [uid, share, role] = s.split(":");
            return {
              user_id: Number(uid),
              share: Number(share),
              role: role || (t.crewRoleDefault ?? "translate"),
            };
          });
        await api.post(`/api/v1/subtitles/requests/${row.id}/claim`, {
          crew: crew.length ? crew : undefined,
        });
      } else if (kind === "deliver") {
        await api.post(`/api/v1/subtitles/requests/${row.id}/deliver`, {
          subtitle_id: Number(input.replace(/\D/g, "")),
        });
      } else {
        // abandon / cancel / accept：纯确认，无输入
        await api.post(
          `/api/v1/subtitles/requests/${row.id}/${kind}`,
          {},
        );
      }
      toast.success(t.wfDone?.replace("{act}", kind) ?? "");
      setAsk(null);
      loadReqs();
    } catch (e) {
      toast.error(apiErrorMessage(dict, e), {
        requestId: e instanceof ApiError ? e.requestId : undefined,
      });
    }
  }

  /** 对话框文案与输入槽（contribute 金额 / claim 协作者 / deliver 字幕 id） */
  const askMeta = (() => {
    if (!ask) return null;
    const ph =
      ask.kind === "contribute"
        ? (t.contribPh?.replace("{magic}", currency) ?? "amount")
        : ask.kind === "claim"
          ? (t.crewPh ?? "crew (uid:share:role, comma, empty=none)")
          : ask.kind === "deliver"
            ? (t.deliverPh ?? "subtitle id")
            : null;
    const msg =
      ask.kind === "abandon"
        ? (t.abandonConfirm ?? "abandon?")
        : ask.kind === "cancel"
          ? (t.cancelConfirm ?? "cancel and refund?")
          : ask.kind === "accept"
            ? (t.acceptConfirm ?? "accept?")
            : null;
    const title = ph ?? msg ?? "";
    return { ph, msg, title };
  })();

  return (
    <section className="mt-6">
      <h2 className="mb-2 text-base font-bold">{t.reqTitle}</h2>
      <form
        className="mb-2 flex flex-wrap items-center gap-2"
        onSubmit={(e) => {
          e.preventDefault();
          void createReq();
        }}
      >
        <select
          value={rLang}
          onChange={(e) => setRLang(e.target.value)}
          aria-label={t.lang}
        >
          <option value="">{t.langSelect}</option>
          {langs.map((l) => (
            <option key={l.code} value={l.code}>
              {l.flag} {l.name}
            </option>
          ))}
        </select>
        <input
          type="text"
          className="w-32"
          value={rBounty}
          onChange={(e) => setRBounty(e.target.value.replace(/\D/g, ""))}
          placeholder={t.reqBountyPh?.replace("{magic}", currency)}
        />
        <label className="flex items-center gap-1 text-sm">
          <input
            type="checkbox"
            checked={rFree}
            onChange={(e) => setRFree(e.target.checked)}
          />
          {t.offerFreeLabel}
        </label>
        <button type="submit" className="btn" disabled={busy}>
          {t.reqBtn}
        </button>
      </form>
      {compact ? (
        <SubtitleRequestCards
          reqs={reqs}
          langs={langs}
          currency={currency}
          meId={me?.id ?? 0}
          meName={me?.username ?? ""}
          wfOn={wfOn}
          onAct={openAsk}
          onContribute={(r) => openAsk("contribute", r)}
        />
      ) : (
        <div className="brand-wide-table-scroll">
        <table className="nexus-table subtitles-list-table">
          <tbody>
            {reqs.map((r) => {
              // 语言码回显人话：与提交下拉同一份 langs 表；未知码回落原值
              const l = langs.find((x) => x.code === r.lang);
              return (
                <tr key={r.id}>
                  <td>{l ? `${l.flag} ${l.name}` : r.lang}</td>
                  <td className="text-sub">{r.username ?? "—"}</td>
                  <td className="num">
                    {r.bounty} {currency}
                    {r.offer_free && (
                      <span className="ml-1 text-[11px] text-mint">Free</span>
                    )}
                  </td>
                  <SubtitleRequestActions
                    r={r}
                    meId={me?.id ?? 0}
                    meName={me?.username ?? ""}
                    wfOn={wfOn}
                    onAct={(a, row) => openAsk(a, row)}
                    onContribute={(id) =>
                      openAsk("contribute", { ...r, id } as SubtitleRequestRow)
                    }
                  />
                </tr>
              );
            })}
            {reqs.length === 0 && (
              <tr>
                <td colSpan={4} className="py-4 text-center text-sub">
                  {t.reqEmpty}
                </td>
              </tr>
            )}
          </tbody>
        </table>
      </div>
      )}

      {/* 工作流/悬赏确认对话框（替代 window.prompt/confirm 家族） */}
      <ConfirmDialog
        open={ask !== null}
        onClose={() => setAsk(null)}
        onConfirm={() => void confirmAsk()}
        title={askMeta?.title ?? ""}
        message={askMeta?.msg ?? undefined}
        confirmLabel={dict.ui.confirmOk}
      >
        {askMeta?.ph && (
          <label className="mt-2 flex flex-col gap-1 text-xs text-sub">
            {askMeta.ph}
            <input
              value={input}
              onChange={(e) => setInput(e.target.value)}
              autoFocus
              className="min-h-[36px] rounded-[var(--r-sm)] border
                border-line bg-[var(--surface-card)] px-3 text-sm text-ink"
            />
          </label>
        )}
      </ConfirmDialog>
    </section>
  );
}
