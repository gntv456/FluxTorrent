/** 发主题选标签（拆自 forum-composer 守门禁）：字典自取
 *  （公开接口 /forums/tags），最多选 5 个。 */

import { useEffect, useState } from "react";
import { api } from "@/lib/api-client";
import { useI18n } from "@/i18n/client";
import { TagChip, TagChipData } from "@/components/forum-bits";

export function TagPicker({
  selected,
  onToggle,
}: {
  selected: number[];
  onToggle: (id: number) => void;
}) {
  const { dict } = useI18n();
  const [dictRows, setDictRows] = useState<TagChipData[]>([]);

  useEffect(() => {
    let alive = true;
    api
      .get<TagChipData[]>("/api/v1/forums/tags")
      .then((r) => {
        if (alive) setDictRows(r ?? []);
      })
      .catch(() => {});
    return () => {
      alive = false;
    };
  }, []);

  if (dictRows.length === 0) return null;
  const t = dict.forums;
  const full = selected.length >= 5;

  return (
    <div className="flex flex-col gap-1">
      <span className="text-sm text-sub">{t.tagPick}</span>
      <div className="flex flex-wrap gap-1.5">
        {dictRows.map((d) => {
          const on = selected.includes(d.id);
          return (
            <button
              key={d.id}
              type="button"
              onClick={() => onToggle(d.id)}
              disabled={!on && full}
              aria-pressed={on}
              className={`rounded-full transition disabled:cursor-not-allowed
                disabled:opacity-40 ${
                  on
                    ? "outline outline-2 outline-offset-1 outline-sky"
                    : "opacity-70 hover:opacity-100"
                }`}
              title={on ? t.tagOn : t.tagOff}
            >
              <TagChip tag={d} />
            </button>
          );
        })}
      </div>
    </div>
  );
}
