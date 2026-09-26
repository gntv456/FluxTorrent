"use client";

import { useEffect, useState } from "react";
import { useSearchParams, useRouter } from "next/navigation";
import { BottomSheet } from "@/components/bottom-sheet";
import { useI18n } from "@/i18n/client";

/** 种子筛选 Bottom Sheet（M3，方案 §3）：四组高频筛选
 *  （存活三态/促销/排序/官种），每选项 = 现有 URL 参数，
 *  确认后 router.replace 双向同步——刷新/分享可复现。
 *  分类/标签等重筛选仍在「高级搜索」面板（桌面同源）；
 *  本 Sheet 是 <md 的快捷面板（FAB 由外层渲染）。 */

export function TorrentFilterSheet({
  open,
  onClose,
}: {
  open: boolean;
  onClose: () => void;
}) {
  const { dict } = useI18n();
  const sp = useSearchParams();
  const router = useRouter();
  const t = dict.torrents2;
  const f = dict.fsheet;

  const cur = {
    alive: sp.get("alive") ?? "",
    promo: sp.get("promo") ?? "",
    sort: sp.get("sort") ?? "",
    official: sp.get("official") ?? "",
  };
  const [sel, setSel] = useState(cur);
  // 打开瞬间同步 URL 态（只在 open 翻转时执行一次；放在 render body 会在
  // 用户点选后把 sel 覆写回 URL 态——M3 验收时「断种选不上」的根因）
  useEffect(() => {
    if (open) setSel(cur);
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [open]);

  const apply = () => {
    const next = new URLSearchParams(sp.toString());
    for (const k of ["alive", "promo", "sort", "official"] as const) {
      if (sel[k]) next.set(k, sel[k]);
      else next.delete(k);
    }
    next.delete("cursor");
    router.replace(`/torrents?${next.toString()}`, { scroll: false });
    onClose();
  };

  const Group = ({
    label,
    k,
    opts,
  }: {
    label: string;
    k: keyof typeof sel;
    opts: readonly (readonly [string, string])[];
  }) => (
    <div className="fsheet__group">
      <p className="fsheet__gtitle">{label}</p>
      <div className="fsheet__chips">
        {opts.map(([v, name]) => (
          <button
            key={v}
            type="button"
            className={`fsheet__chip ${sel[k] === v ? "is-on" : ""}`}
            onClick={() => setSel((s) => ({ ...s, [k]: v }))}
          >
            {name}
          </button>
        ))}
      </div>
    </div>
  );

  return (
    <BottomSheet open={open} onClose={onClose} title={t.advanced}>
      <Group
        label={f.alive}
        k="alive"
        opts={[
          ["", f.all],
          ["1", f.aliveOnly],
          ["2", f.deadOnly],
        ]}
      />
      <Group
        label={f.promo}
        k="promo"
        opts={[
          ["", f.all],
          ["free", f.free],
          ["2x", f.x2],
          ["half", f.half],
        ]}
      />
      <Group
        label={f.official}
        k="official"
        opts={[
          ["", f.any],
          ["1", f.officialOnly],
        ]}
      />
      <Group label={f.sort}
        k="sort"
        opts={[
          ["", f.sortDefault],
          ["seeders", f.sortSeeders],
          ["size", f.sortSize],
          ["created", f.sortCreated],
        ]} />
      <div className="fsheet__acts">
        <button type="button" className="fsheet__btn" onClick={onClose}>
          {dict.common.cancel}
        </button>
        <button
          type="button"
          className="fsheet__btn is-primary"
          onClick={apply}
        >
          {t.searchBtn}
        </button>
      </div>
    </BottomSheet>
  );
}
