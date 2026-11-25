"use client";
import { useI18n } from "@/i18n/client";

/** AI 房大厅：底分选档。模式/变体切换由顶部 ModeSwitch 统一承载，这里不重复。 */
export function AiLobby({
  ruleLine,
  stakeTiers,
  busy,
  onStart,
}: {
  ruleLine: string;
  stakeTiers: number[];
  busy: boolean;
  onStart: (stake: number) => void;
}) {
  const { dict } = useI18n();
  const t = dict.games.doudizhu;
  return (
    <div className="dz-lobby">
      <h2 className="dz-h2">{t.startTitle}</h2>
      <p className="dz-rule-line">{ruleLine}</p>
      <div className="dz-tiers">
        {stakeTiers.map((s) => (
          <button
            key={s}
            type="button"
            className="dz-tier"
            disabled={busy}
            onClick={() => onStart(s)}
          >
            <strong>{t.stakeTier.replace("{n}", String(s))}</strong>
            <span>{t.stakeSub}</span>
          </button>
        ))}
      </div>
    </div>
  );
}

/** PvP 大厅头部的模式切换条（AI/PvP + 经典/癞子）。 */
export function ModeSwitch({
  mode,
  variant,
  busy,
  onMode,
  onVariant,
}: {
  mode: "ai" | "pvp";
  variant: "classic" | "wild";
  busy: boolean;
  onMode: (m: "ai" | "pvp") => void;
  onVariant: (v: "classic" | "wild") => void;
}) {
  const { dict } = useI18n();
  const t = dict.games.doudizhu;
  return (
    <div className="dz-controls" style={{ marginBottom: 12 }}>
      {(["ai", "pvp"] as const).map((m) => (
        <button
          key={m}
          type="button"
          className={`dz-btn${mode === m ? " is-primary" : ""}`}
          disabled={busy}
          onClick={() => onMode(m)}
        >
          {m === "ai" ? t.aiTab : t.pvpTab}
        </button>
      ))}
      {(["classic", "wild"] as const).map((v) => (
        <button
          key={v}
          type="button"
          className={`dz-btn${variant === v ? " is-primary" : ""}`}
          disabled={busy}
          onClick={() => onVariant(v)}
        >
          {v === "classic" ? t.modeClassic : t.modeWild}
        </button>
      ))}
    </div>
  );
}
