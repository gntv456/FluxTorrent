"use client";

import { useState } from "react";
import { api, ApiError } from "@/lib/api-client";
import { useI18n } from "@/i18n/client";
import { petLineOf, PET_SPECIES, PET_SPECIES_KEY } from "@/lib/pet-lines";
import type { PetSpecies } from "@/lib/pet-lines";

export interface PetStatus {
  species: string;
  name: string;
  level: number;
  exp: number;
  exp_to_next: number;
  hunger: number;
  energy: number;
  pending: number;
  yield_permille: number;
  feed_cost: number;
  digest_per_hour: number;
  max_level: number;
  /** 口粮券余额（行为联动：每日做种满 6h 得 1 张，可抵一次投喂） */
  food_coupons?: number;
}

/** 宠物栏：成长形象 + 状态条。纯展示，动作在页面。 */
export function PetPen({
  status,
  hungerLabel,
  expLabel,
}: {
  status: PetStatus;
  hungerLabel: string;
  expLabel: string;
}) {
  const { dict } = useI18n();
  const line = petLineOf(status.species);
  const i = Math.min(Math.max(status.level - 1, 0), line.length - 1);
  const maxed = status.level >= status.max_level;
  const spKey =
    PET_SPECIES_KEY[status.species as PetSpecies] ?? "spSlime";
  return (
    <div className="pp-pen">
      {status.name ? <div className="pp-name num">{status.name}</div> : null}
      <div className="pp-stage">
        <span className={`pp-pet${maxed ? " max" : ""}`} aria-hidden>
          {line[i]}
        </span>
        <span className="pp-ring" aria-hidden />
      </div>
      <div className="pp-species-tag">
        {(dict.games.pet as Record<string, string>)[spKey]}
      </div>
      <div className="pp-bars">
        <div className="pp-bar">
          <span className="pp-bar-lb">{hungerLabel}</span>
          <span className="pp-track">
            <i
              className="pp-fill hunger"
              style={{ width: `${Math.max(0, Math.min(100, status.hunger))}%` }}
            />
          </span>
          <span className="num pp-bar-n">{status.hunger}%</span>
        </div>
        <div className="pp-bar">
          <span className="pp-bar-lb">{expLabel}</span>
          <span className="pp-track">
            <i
              className="pp-fill exp"
              style={{ width: `${maxed ? 100 : status.exp % 100}%` }}
            />
          </span>
          <span className="num pp-bar-n">
            {maxed ? "MAX" : `${status.exp % 100}/100`}
          </span>
        </div>
      </div>
    </div>
  );
}

/** 宠物档案：起名 + 选种（只改外观）。保存后把最新状态交回页面。 */
export function PetCustom({
  status,
  onClose,
  onSaved,
}: {
  status: PetStatus;
  onClose: () => void;
  onSaved: (v: { name: string; species: string }) => void;
}) {
  const { dict } = useI18n();
  const tp = dict.games.pet;
  const [name, setName] = useState(status.name);
  const [species, setSpecies] = useState(status.species);
  const [busy, setBusy] = useState(false);
  const [err, setErr] = useState<string | null>(null);

  async function save() {
    if (busy) return;
    setBusy(true);
    setErr(null);
    try {
      const r = await api.post<{ name: string; species: string }>(
        "/api/v1/games/pet/customize",
        { name, species },
      );
      onSaved(r);
    } catch (e) {
      setErr(
        e instanceof ApiError
          ? e.code === 1002
            ? e.message
            : (dict.errors[e.code] ?? e.message)
          : dict.common.networkError,
      );
      setBusy(false);
    }
  }

  return (
    <div className="pp-custom-wrap" role="presentation">
    <div className="pp-custom" role="dialog" aria-label={tp.customTitle}>
      <div className="pp-custom-hd">
        <h4>{tp.customTitle}</h4>
        <button type="button" className="pp-custom-x" onClick={onClose}>
          ✕
        </button>
      </div>
      <label className="pp-custom-lb" htmlFor="pet-name">
        {tp.nameLabel}
      </label>
      <input
        id="pet-name"
        className="pp-custom-input"
        value={name}
        placeholder={tp.namePh}
        maxLength={12}
        onChange={(e) => setName(e.target.value)}
      />
      <div className="pp-custom-lb">{tp.speciesLabel}</div>
      <div className="pp-species">
        {PET_SPECIES.map((sp) => {
          const line = petLineOf(sp);
          return (
            <button
              key={sp}
              type="button"
              onClick={() => setSpecies(sp)}
              className={`pp-sp${species === sp ? " on" : ""}`}
              aria-pressed={species === sp}
            >
              <span className="pp-sp-line" aria-hidden>
                {line[2]}→{line[9]}
              </span>
              <span className="pp-sp-name">
                {(tp as Record<string, string>)[PET_SPECIES_KEY[sp]]}
              </span>
            </button>
          );
        })}
      </div>
      {err && <p className="text-xs text-danger">{err}</p>}
      <button
        type="button"
        className="arc-call sky pp-custom-save"
        onClick={save}
        disabled={busy}
      >
        {busy ? tp.saving : tp.save}
      </button>
    </div>
    </div>
  );
}
