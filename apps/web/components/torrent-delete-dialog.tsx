"use client";

import { useEffect, useState } from "react";

import { Modal } from "@/components/modal";
import { api, ApiError } from "@/lib/api-client";
import { useI18n } from "@/i18n/client";
import {
  BTN_SM_BOLD,
  BTN_SM_DANGER,
  BTN_SM_GHOST,
} from "@/lib/ui-classes";

/** 站方删除表单（NP delete.php 口径）：软删之外补上「理由 + 三项处置」。
 *
 *  取数走 GET /torrents/{id}/delete-preview——同伴计数与「发种人从这一颗种
 *  入袋的 {magic}」都由后端算，前端不自估（自估必然与回收口漂移）。
 *  `can_dispose` 与写口那句资格判据逐字一致，所以界面给得出的勾，提交必然
 *  收得下；作者自删未过审的种只有一条删除路径，处置块整块换成一句说明。
 *
 *  删除成功但处置失败＝种子已经没了，不能只 console 一声：翻成
 *  「已删除，但下列处置未成功」清单，操作员读到了才离开本页。 */

const REASONS = [
  "dead",
  "duplicate",
  "low_quality",
  "violation",
  "other",
];
/** 与后端 NOTE_REQUIRED 同一清单：这两项语义不足以复盘裁决 */
const NOTE_REQUIRED = ["violation", "other"];

const FIELD =
  "mt-1 w-full rounded-[var(--r-sm)] border border-line " +
  "bg-transparent px-2 py-1 text-xs text-ink";

type Preview = {
  owner_name: string;
  seeders: number;
  leechers: number;
  peers: number;
  spark: number;
  upload_banned: boolean;
  can_dispose: boolean;
};

type DelResp = {
  deleted: number;
  sanctions?: { errors?: string[] };
};

export function TorrentDeleteDialog({
  open,
  torrentId,
  onClose,
  onDeleted,
}: {
  open: boolean;
  torrentId: number;
  onClose: () => void;
  /** 删除已成立（含处置部分失败后操作员确认知悉）：关窗并离开本页 */
  onDeleted: () => void;
}) {
  const { dict, currency } = useI18n();
  const t = dict.torrentManage2;
  const [pv, setPv] = useState<Preview | null>(null);
  const [reason, setReason] = useState("dead");
  const [note, setNote] = useState("");
  const [deduct, setDeduct] = useState(false);
  const [dbl, setDbl] = useState(false);
  const [ban, setBan] = useState(false);
  const [busy, setBusy] = useState(false);
  const [err, setErr] = useState<string | null>(null);
  // 非空＝删除已落库、但处置有失败项，必须先让操作员读完
  const [partial, setPartial] = useState<string[] | null>(null);

  const labels: Record<string, string> = {
    dead: t.delReasonDead,
    duplicate: t.delReasonDuplicate,
    low_quality: t.delReasonLowQuality,
    violation: t.delReasonViolation,
    other: t.delReasonOther,
  };

  useEffect(() => {
    if (!open) return;
    let live = true;
    setPv(null);
    setErr(null);
    setPartial(null);
    setNote("");
    setDeduct(false);
    setDbl(false);
    setBan(false);
    api
      .get<Preview>(`/api/v1/torrents/${torrentId}/delete-preview`)
      .then((v) => live && setPv(v))
      // 取数失败不拦住删除（处置块自然不出现），但必须出声——
      // 静默回落成「没有可回收的 {magic}」会瞒住一次本该发生的处置
      .catch((e) => {
        if (!live) return;
        setPv(null);
        setErr(e instanceof ApiError ? e.message : t.delLoadFail);
      });
    // t 是字典快照（整页语言切换会重挂），不必进依赖
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [open, torrentId]);

  const needNote = NOTE_REQUIRED.includes(reason) && !note.trim();

  async function submit() {
    setBusy(true);
    setErr(null);
    try {
      const r = await api.del<DelResp>(`/api/v1/torrents/${torrentId}`, {
        reason,
        note: note.trim(),
        deduct_spark: deduct,
        double_spark: dbl,
        ban_upload: ban,
      });
      const bad = r?.sanctions?.errors ?? [];
      if (bad.length) setPartial(bad);
      else onDeleted();
    } catch (e) {
      setErr(e instanceof ApiError ? e.message : t.delFail);
    } finally {
      setBusy(false);
    }
  }

  return (
    <Modal open={open} onClose={onClose} title={t.delTitle}>
      {partial ? (
        <span className="flex flex-col gap-2">
          <p className="text-xs font-bold">{t.delPartial}</p>
          <ul className="list-disc pl-4 text-xs">
            {partial.map((x, i) => (
              <li key={i}>{x}</li>
            ))}
          </ul>
          <span className="flex justify-end">
            <button
              type="button"
              className={BTN_SM_BOLD}
              onClick={onDeleted}
            >
              {t.delGotIt}
            </button>
          </span>
        </span>
      ) : (
        <span className="flex flex-col gap-2">
          <p className="text-xs">{t.delConfirm}</p>
          {pv && (
            <p className="text-xs text-sub">
              {t.delPeers
                .replace("{seed}", String(pv.seeders))
                .replace("{leech}", String(pv.leechers))
                .replace("{peers}", String(pv.peers))}
            </p>
          )}
          <label className="text-xs font-bold">
            {t.delReason}
            <select
              className={FIELD}
              value={reason}
              onChange={(e) => setReason(e.target.value)}
            >
              {REASONS.map((r) => (
                <option key={r} value={r}>
                  {labels[r]}
                  {NOTE_REQUIRED.includes(r) ? t.delRequired : ""}
                </option>
              ))}
            </select>
          </label>
          <label className="text-xs font-bold">
            {t.delNote}
            <textarea
              className={FIELD}
              rows={3}
              maxLength={500}
              value={note}
              onChange={(e) => setNote(e.target.value)}
            />
          </label>
          {pv?.can_dispose ? (
            <span className="flex flex-col gap-1">
              <label className="flex items-center gap-2 text-xs">
                <input
                  type="checkbox"
                  checked={deduct}
                  disabled={pv.spark <= 0}
                  onChange={(e) => {
                    setDeduct(e.target.checked);
                    if (!e.target.checked) setDbl(false);
                  }}
                />
                {t.delSpark.replaceAll("{magic}", currency)}
                <span className="text-sub font-normal">
                  {t.delSparkNote
                    .replace("{n}", String(pv.spark))
                    .replaceAll("{magic}", currency)}
                </span>
              </label>
              <label className="flex items-center gap-2 pl-6 text-xs">
                <input
                  type="checkbox"
                  checked={dbl}
                  disabled={!deduct}
                  onChange={(e) => setDbl(e.target.checked)}
                />
                {t.delDouble.replaceAll("{magic}", currency)}
                <span className="text-sub font-normal">
                  {t.delDoubleNote.replace(
                    "{n}",
                    String(pv.spark * 2),
                  )}
                </span>
              </label>
              <label className="flex items-center gap-2 text-xs">
                <input
                  type="checkbox"
                  checked={ban}
                  disabled={pv.upload_banned}
                  onChange={(e) => setBan(e.target.checked)}
                />
                {t.delBan}
                <span className="text-sub font-normal">
                  {pv.upload_banned ? t.delBanOn : t.delBanNote}
                </span>
              </label>
            </span>
          ) : (
            <p className="text-xs text-sub">{t.delNoDispose}</p>
          )}
          {err && <p className="text-xs">{err}</p>}
          <span className="flex items-center justify-end gap-2">
            <button
              type="button"
              className={BTN_SM_GHOST}
              onClick={onClose}
              disabled={busy}
            >
              {dict.common.cancel}
            </button>
            <button
              type="button"
              className={BTN_SM_DANGER}
              disabled={busy || needNote}
              onClick={() => void submit()}
            >
              {t.delOk}
            </button>
          </span>
        </span>
      )}
    </Modal>
  );
}
