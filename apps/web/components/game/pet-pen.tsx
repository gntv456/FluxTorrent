"use client";

import { useEffect, useState } from "react";
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

/** 宠物情绪：饿=馋（投喂可解锁动画），饱=开心；满级再叠一层 celebrating */
function moodOf(status: PetStatus): "hungry" | "happy" | "max" {
  if (status.level >= status.max_level) return "max";
  return status.hunger < 40 ? "hungry" : "happy";
}

/** 宠物栏：小舞台（天光/地面/气泡/等级徽章）+ 成长形象 + 状态条。
 *  纯展示；「摸摸头」是本地动画（零账目，纯情感反馈）。 */
export function PetPen({
  status,
  eating = false,
  patTick = 0,
  hungerLabel,
  expLabel,
}: {
  status: PetStatus;
  eating?: boolean;
  /** 外部「摸摸头」按钮递增的计数：变化即播一次爱心（受控，无 DOM 代理） */
  patTick?: number;
  hungerLabel: string;
  expLabel: string;
}) {
  const { dict } = useI18n();
  const tp = dict.games.pet as Record<string, string>;
  const [pat, setPat] = useState(0);
  useEffect(() => {
    if (patTick > 0) setPat((n) => n + 1);
  }, [patTick]);
  const line = petLineOf(status.species);
  const i = Math.min(Math.max(status.level - 1, 0), line.length - 1);
  // 进化演出（趣味性批）：升级瞬间换 key 触发 pp-evolve 弹入；
  // 判据 = 等级超出上一档门槛（向前回看一格，跨档即换形态）
  const prevI = Math.min(Math.max(status.level - 2, 0), line.length - 1);
  const evolved = i !== prevI && status.level > 1;
  const maxed = status.level >= status.max_level;
  const mood = moodOf(status);
  const spKey = PET_SPECIES_KEY[status.species as PetSpecies] ?? "spSlime";
  return (
    <div className="pp-pen">
      {status.name ? <div className="pp-name num">{status.name}</div> : null}
      <div className={`pp-stage mood-${mood}`}>
        <span className="pp-glow" aria-hidden />
        <span
          key={i}
          className={`pp-pet${maxed ? " max" : ""}${eating ? " eating" : ""}${
            evolved ? " pp-evolve" : ""
          }`}
          aria-hidden
        >
          {line[i]}
        </span>
        <span className="pp-lv num" aria-label={`Lv.${status.level}`}>
          {status.level}
        </span>
        <span className="pp-bubble" aria-hidden>
          {mood === "hungry" ? tp.moodHungry : tp.moodHappy}
        </span>
        <span className="pp-ground" aria-hidden />
        <span className="pp-ground-shadow" aria-hidden />
        <button
          type="button"
          className="pp-pat"
          aria-label={tp.pat}
          title={tp.pat}
          onClick={() => setPat((n) => n + 1)}
        >
          <span key={pat} className="pp-pat-heart" aria-hidden>
            {pat > 0 ? "💜" : ""}
          </span>
        </button>
      </div>
      <div className="pp-species-tag">
        {tp[spKey]}
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
