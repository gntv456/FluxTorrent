"use client";

import { useCallback, useEffect, useState } from "react";
import { api, ApiError, rawFetchHelpers } from "@/lib/api-client";
import { useI18n } from "@/i18n/client";
import type { SubtitleRequestRow } from "@/components/subtitle-board-shared";

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
  const [me, setMe] = useState<{ id: number } | null>(null);
  // 工作流开关（site-profile 暴露；关 = NP 直交付口径）
  const [wfOn, setWfOn] = useState(false);

  const loadReqs = useCallback(async () => {
    try {
      setReqs(
        await api.get<SubtitleRequestRow[]>(
          "/api/v1/subtitles/requests?status=open",
        ),
      );
    } catch {
      setReqs([]);
    }
  }, []);

  useEffect(() => {
    loadReqs();
    api
      .get<{ id: number }>("/api/v1/me")
      .then(setMe)
      .catch(() => setMe(null));
    fetch("/api/v1/site-profile")
      .then((r) => r.json())
      .then((b: { data?: { subtitle_workflow?: boolean } }) =>
        setWfOn(!!b.data?.subtitle_workflow),
      )
      .catch(() => {});
  }, [loadReqs, reloadKey]);

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
      onMsg(t.reqOk?.replace("{magic}", currency) ?? "request created");
      loadReqs();
    } catch (e) {
      onMsg(e instanceof ApiError ? e.message : dict.common.networkError);
    } finally {
      setBusy(false);
    }
  }

  async function contribute(id: number) {
    const v = window.prompt(
      t.contribPh?.replace("{magic}", currency) ?? "amount",
    );
    if (!v) return;
    try {
      await api.post(`/api/v1/subtitles/requests/${id}/contribute`, {
        amount: Number(v.replace(/\D/g, "")) || 0,
      });
      loadReqs();
    } catch (e) {
      onMsg(e instanceof Error ? e.message : "failed");
    }
  }

  /** 工作流动作（0148）：认领（可带协作者）/弃单/交稿/发起人验收 */
  async function wfAction(
    act: "claim" | "abandon" | "deliver" | "accept",
    r: SubtitleRequestRow,
  ) {
    try {
      if (act === "claim") {
        const crewRaw = window.prompt(
          t.crewPh ?? "crew (uid:share:role, comma, empty=none)",
        );
        const crew = (crewRaw ?? "")
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
        await api.post(`/api/v1/subtitles/requests/${r.id}/claim`, {
          crew: crew.length ? crew : undefined,
        });
      } else if (act === "deliver") {
        const sid = window.prompt(t.deliverPh ?? "subtitle id");
        if (!sid) return;
        await api.post(`/api/v1/subtitles/requests/${r.id}/deliver`, {
          subtitle_id: Number(sid.replace(/\D/g, "")),
        });
      } else {
        if (!window.confirm(t.acceptConfirm ?? "accept?")) return;
        await api.post(`/api/v1/subtitles/requests/${r.id}/accept`, {});
      }
      onMsg(t.wfDone?.replace("{act}", act) ?? "done");
      loadReqs();
    } catch (e) {
      onMsg(e instanceof Error ? e.message : "failed");
    }
  }

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
      <div className="baozi-wide-table-scroll">
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
                  <td>
                    <button
                      type="button"
                      className="btn2"
                      onClick={() => void contribute(r.id)}
                    >
                      {t.contributeBtn}
                    </button>
                    {wfOn && r.status === 0 && (
                      <button
                        type="button"
                        className="btn2 ml-1"
                        onClick={() => void wfAction("claim", r)}
                      >
                        {t.claimBtn}
                      </button>
                    )}
                    {wfOn && r.status === 3 && r.claimed_by === me?.id && (
                      <>
                        <button
                          type="button"
                          className="btn2 ml-1"
                          onClick={() => void wfAction("deliver", r)}
                        >
                          {t.deliverBtn}
                        </button>
                        <button
                          type="button"
                          className="btn2 ml-1"
                          onClick={() => void wfAction("abandon", r)}
                        >
                          {t.abandonBtn}
                        </button>
                      </>
                    )}
                    {wfOn && r.status === 4 && r.username && (
                      <button
                        type="button"
                        className="btn2 ml-1"
                        onClick={() => void wfAction("accept", r)}
                      >
                        {t.acceptBtn}
                      </button>
                    )}
                  </td>
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
    </section>
  );
}

/** 附件上传共用：multipart 到 attachments 拿 sha256（board 与种子面板复用） */
export async function uploadAttachment(file: File): Promise<string> {
  const form = new FormData();
  form.append("file", file);
  const lang = rawFetchHelpers.lang();
  const upRes = await fetch(rawFetchHelpers.base() + "/api/v1/attachments", {
    method: "POST",
    headers: {
      ...(lang ? { "Accept-Language": lang } : {}),
    },
    body: form,
  });
  const upBody = (await upRes.json()) as {
    code: number;
    message?: string;
    data?: { sha256: string };
  };
  if (upBody.code !== 0 || !upBody.data?.sha256) {
    throw new ApiError(upBody.code, upBody.message ?? "upload failed");
  }
  return upBody.data.sha256;
}
