"use client";

import { useCallback, useEffect, useState } from "react";
import Link from "next/link";
import { api, ApiError } from "@/lib/api-client";
import { useI18n } from "@/i18n/client";
import { fmt } from "@/i18n/config";
import { BTN_SM_BOLD } from "@/lib/ui-classes";
import { Row } from "@/components/usercp-row";

/** 自定义头衔兑换面（0295，运营轮 P0-5）。
 *
 *  背景：商店 80000 魔力的「自定义头衔」SKU（shop_items.id=7，
 *  config `{"unlock": true}`）自 0292 起改为发 title_unlock 券，核销端点
 *  POST /me/title 也一直在——但**前端没有任何一处调用过它**（全仓 grep
 *  `me/title` 0 命中）。于是买完券的人没有入口用，只能看着它过期：
 *  钱照扣、能力拿不到，且界面上看不出哪里出了问题。
 *
 *  口径（与后端 0295 同步）：一张券换一次「设置/清空头衔」，
 *  已核销或已过期的券不能用；手上还有可用券时才允许提交。
 *  券状态一律读 GET /me/title（服务端用 has_usable_voucher 同一条判据）。 */

interface TitleState {
  title: string | null;
  usable: number;
  expires_at: string | null;
}

const FLD =
  "w-full max-w-md rounded-[var(--r-sm)] border border-line " +
  "bg-[var(--baozi-bg-input)] px-2 py-1 text-sm text-ink";

export function TitleRow() {
  const { dict, locale } = useI18n();
  const t = dict.usercp.personal;
  const [st, setSt] = useState<TitleState | null>(null);
  const [text, setText] = useState("");
  const [msg, setMsg] = useState<string | null>(null);
  const [err, setErr] = useState<string | null>(null);
  const [busy, setBusy] = useState(false);

  const load = useCallback(async () => {
    try {
      const s = await api.get<TitleState>("/api/v1/me/title");
      setSt(s);
      setText(s.title ?? "");
    } catch {
      // 读不到状态就按「没有券」呈现：宁可不给提交，也不要让人误扣
      setSt({ title: null, usable: 0, expires_at: null });
    }
  }, []);

  useEffect(() => {
    void load();
  }, [load]);

  async function save() {
    if (!st || st.usable <= 0 || busy) return;
    setBusy(true);
    setMsg(null);
    setErr(null);
    try {
      await api.post<{ title: string }>("/api/v1/me/title", {
        title: text.trim(),
      });
      setMsg(t.titleNote);
      await load();
    } catch (e) {
      setErr(
        e instanceof ApiError
          ? (dict.errors[e.code] ?? e.message)
          : t.titleFailed,
      );
    } finally {
      setBusy(false);
    }
  }

  if (!st) return <p className="py-2 text-sm text-sub">{dict.my.loading}</p>;

  const canSave = st.usable > 0 && !busy && text.trim().length <= 30;
  const until = st.expires_at
    ? new Date(st.expires_at).toLocaleDateString(locale)
    : "";

  return (
    <Row head={t.titleLabel}>
      <input
        type="text"
        className={FLD}
        maxLength={30}
        value={text}
        placeholder={t.titleNote}
        disabled={st.usable <= 0}
        onChange={(e) => setText(e.target.value)}
      />
      <br />
      <button
        type="button"
        className={BTN_SM_BOLD}
        disabled={!canSave}
        onClick={() => void save()}
      >
        {t.titleSave}
      </button>{" "}
      <span className="text-xs text-sub">
        {st.usable > 0
          ? fmt(t.titleVoucherValid, { n: st.usable, date: until })
          : t.titleVoucherNone}
      </span>
      {st.usable <= 0 && (
        <>
          {" "}
          <Link href="/shop" className="text-xs">
            {t.titleBuy}
          </Link>
        </>
      )}
      <br />
      <span className="uc-note">
        <b>{dict.usercp.note}</b>
        {t.titleCost}
      </span>
      {err && <p className="mt-1 text-xs text-danger">{err}</p>}
      {msg && <p className="mt-1 text-xs text-sub">{msg}</p>}
    </Row>
  );
}
