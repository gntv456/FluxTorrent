"use client";

import { useEffect, useState } from "react";
import { useI18n } from "@/i18n/client";

/** 字幕面板语言选择（从 torrent-subtitles.tsx 挪出，300 行门禁腾位）：
 *  读字典；失败回落常见六语。 */
export function LangSelect({
  value,
  onChange,
}: {
  value: string;
  onChange: (v: string) => void;
}) {
  const { dict } = useI18n();
  const t = dict.subtitles;
  const s = t.langSelect ?? "(select)";
  const [langs, setLangs] = useState<
    { code: string; name: string; flag: string }[]
  >([
    { code: "chs", name: t.langChs, flag: "🇨🇳" },
    { code: "cht", name: t.langCht, flag: "🇹🇼" },
    { code: "eng", name: t.langEng, flag: "🇬🇧" },
    { code: "jpn", name: t.langJpn, flag: "🇯🇵" },
    { code: "kor", name: t.langKor, flag: "🇰🇷" },
    { code: "other", name: t.langOther, flag: "🌐" },
  ]);
  useEffect(() => {
    import("@/components/subtitle-board-format")
      .then((m) => m.loadLangDict())
      .then((d) => {
        if (d.length) setLangs(d);
      })
      .catch(() => {});
  }, []);
  return (
    <select value={value} onChange={(e) => onChange(e.target.value)}>
      <option value="">{s}</option>
      {langs.map((l) => (
        <option key={l.code} value={l.code}>
          {l.flag} {l.name}
        </option>
      ))}
    </select>
  );
}
