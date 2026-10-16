"use client";

import { useEffect, useState } from "react";

import { Modal } from "@/components/modal";
import { api, ApiError } from "@/lib/api-client";
import { useI18n } from "@/i18n/client";
import { BTN_SM_DANGER, BTN_SM_GHOST } from "@/lib/ui-classes";
import type { DenyReason } from "./admin-torrents-shared";

/** 拒绝种子的理由表单（后台审核）。
 *
 *  旧形态是 `prompt()` 把 `1. xxx / 2. xxx` 拼成一串文本让版主「输入编号」，
 *  编号填错就静默把整句当成自定义理由发出去（`admin-torrents-list-parts.tsx`
 *  原 :36-38）。理由字典（torrent_deny_reasons，本站 7 条）本来就是给结构化
 *  裁决用的，走输入框等于放弃结构化；而且原生 prompt 在部分浏览器里根本
 *  不可用。
 *
 *  规则：选字典项即可提交；选「其他」或本站还没配字典 ⇒ 备注必填。
 *  字典读失败不拦住拒绝（备注照样能提交），但把「清单没读到」写在框里——
 *  回落成空列表会被读成「本站没配理由」。 */

/** 「其他（自己写）」的伪 id，与字典 id 不冲突 */
const OTHER = -1;

const FIELD =
  "mt-1 w-full rounded-[var(--r-sm)] border border-line " +
  "bg-transparent px-2 py-1 text-xs text-ink";

export function AdminDenyDialog({
  open,
  torrentId,
  onClose,
  onDone,
}: {
  open: boolean;
  torrentId: number;
  onClose: () => void;
  /** 拒绝已成立：带回给列表的一句提示 */
  onDone: (msg: string) => void;
}) {
  const { dict } = useI18n();
  const at = dict.adminTorrents;
  const [rows, setRows] = useState<DenyReason[] | null>(null);
  const [loadErr, setLoadErr] = useState<string | null>(null);
  const [picked, setPicked] = useState<number | null>(null);
  const [note, setNote] = useState("");
  const [busy, setBusy] = useState(false);
  const [err, setErr] = useState<string | null>(null);

  useEffect(() => {
    if (!open) return;
    let live = true;
    setRows(null);
    setLoadErr(null);
    setPicked(null);
    setNote("");
    setErr(null);
    api
      .get<DenyReason[]>("/api/v1/admin/deny-reasons")
      .then((r) => {
        if (live) setRows(r ?? []);
      })
      .catch((e) => {
        if (!live) return;
        setRows([]);
        setLoadErr(e instanceof ApiError ? e.message : at.denyLoadFail);
      });
    // at 是字典快照（整页换语言会重挂），不必进依赖
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [open, torrentId]);

  const list = rows ?? [];
  const failed = loadErr !== null;
  const empty = list.length === 0;
  // 没配字典 / 清单没读到 / 选了「其他」——这三种都必须落一句自写备注。
  // 但只有第一种能在界面上说「本站还没配拒绝原因」：把读失败也说成没配置，
  // 就是拿假解释盖住真故障。
  const needNote = failed || empty || picked === OTHER;
  const okToSubmit = busy
    ? false
    : needNote
      ? note.trim().length > 0
      : picked !== null;

  async function submit() {
    setBusy(true);
    setErr(null);
    try {
      await api.post("/api/v1/admin/reviews/decide", {
        torrent_id: torrentId,
        approve: false,
        reason: note.trim(),
        deny_reason_id:
          picked === null || picked === OTHER ? undefined : picked,
      });
      onDone(at.rejectedMsg.replace("{id}", String(torrentId)));
    } catch (e) {
      setErr(e instanceof ApiError ? e.message : at.opFail);
    } finally {
      setBusy(false);
    }
  }

  return (
    <Modal open={open} onClose={onClose} title={at.denyTitle}>
      <span className="flex flex-col gap-2">
        <p className="text-xs text-sub">
          {at.denyFor.replace("{id}", String(torrentId))}
        </p>
        {failed && (
          <p className="text-xs">
            {at.denyLoadFail}：{loadErr}
          </p>
        )}
        {!failed && empty ? (
          <p className="text-xs text-sub">{at.denyEmptyDict}</p>
        ) : (
          <span className="flex flex-col gap-1">
            <span className="text-xs font-bold">{at.denyPick}</span>
            {list.map((r) => (
              <label
                key={r.id}
                className="flex items-center gap-2 text-xs"
              >
                <input
                  type="radio"
                  name="deny-reason"
                  checked={picked === r.id}
                  onChange={() => setPicked(r.id)}
                />
                {r.reason}
              </label>
            ))}
            <label className="flex items-center gap-2 text-xs">
              <input
                type="radio"
                name="deny-reason"
                checked={picked === OTHER}
                onChange={() => setPicked(OTHER)}
              />
              {at.denyOther}
            </label>
          </span>
        )}
        <label className="text-xs font-bold">
          {at.denyNoteLabel}
          {needNote ? at.denyRequired : at.denyNoteOptional}
          <textarea
            className={FIELD}
            rows={3}
            maxLength={500}
            value={note}
            onChange={(e) => setNote(e.target.value)}
          />
        </label>
        {needNote && !note.trim() && (
          <p className="text-xs">{at.denyNoteShort}</p>
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
            disabled={!okToSubmit}
            onClick={() => void submit()}
          >
            {at.denySubmit}
          </button>
        </span>
      </span>
    </Modal>
  );
}
