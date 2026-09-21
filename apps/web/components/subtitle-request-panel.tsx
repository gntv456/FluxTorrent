"use client";

import { useCallback, useEffect, useState } from "react";
import { api, ApiError, rawFetchHelpers } from "@/lib/api-client";
import { useI18n } from "@/i18n/client";
import type { SubtitleRequestRow } from "@/components/subtitle-board-shared";

/** 求字幕悬赏（pots，0146 P2-1）：发起 + 入池列表。
 *  从 subtitle-board.tsx 按域拆出（300 行门禁）。 */

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
  const [busy, setBusy] = useState(false);

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
  }, [loadReqs, reloadKey]);

  async function createReq() {
    if (!rLang) return;
    setBusy(true);
    try {
      await api.post("/api/v1/subtitles/requests", {
        lang: rLang,
        torrent_id: fixedTorrentId ?? undefined,
        bounty: rBounty ? Number(rBounty) : 0,
      });
      setRLang("");
      setRBounty("");
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
        <button type="submit" className="btn" disabled={busy}>
          {t.reqBtn}
        </button>
      </form>
      <table className="nexus-table subtitles-list-table">
        <tbody>
          {reqs.map((r) => (
            <tr key={r.id}>
              <td>{r.lang}</td>
              <td className="text-sub">{r.username ?? "—"}</td>
              <td className="num">
                {r.bounty} {currency}
              </td>
              <td>
                <button
                  type="button"
                  className="btn2"
                  onClick={() => void contribute(r.id)}
                >
                  {t.contributeBtn}
                </button>
              </td>
            </tr>
          ))}
          {reqs.length === 0 && (
            <tr>
              <td colSpan={4} className="py-4 text-center text-sub">
                {t.reqEmpty}
              </td>
            </tr>
          )}
        </tbody>
      </table>
    </section>
  );
}

/** 附件上传共用：multipart 到 attachments 拿 sha256（board 与种子面板复用） */
export async function uploadAttachment(
  file: File,
): Promise<string> {
  const form = new FormData();
  form.append("file", file);
  const lang = rawFetchHelpers.lang();
  const upRes = await fetch(
    rawFetchHelpers.base() + "/api/v1/attachments",
    {
      method: "POST",
      headers: {
        ...(lang ? { "Accept-Language": lang } : {}),
      },
      body: form,
    },
  );
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
