"use client";
import { useI18n } from "@/i18n/client";

/** AI 房大厅：底分选档 + 变体切换（经典/癞子）+ 去 PvP 入口。 */
export function AiLobby({
  ruleLine,
  stakeTiers,
  variant,
  busy,
  onVariant,
  onGoPvp,
  onStart,
}: {
  ruleLine: string;
  stakeTiers: number[];
  variant: "classic" | "wild";
  busy: boolean;
  onVariant: (v: "classic" | "wild") => void;
  onGoPvp: () => void;
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
      <div className="dz-controls" style={{ marginTop: 12 }}>
        <button
          type="button"
          className="dz-btn"
          disabled={busy}
          onClick={onGoPvp}
        >
          {t.pvpTab}
        </button>
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
