"use client";

/**
 * 农场子页批（样图⑤ 2026-10 补页）：牧场 / 加工坊 / 图鉴 三个分区件。
 * 数据来自 /farm/ranch 与 /farm/collection；动作走 buy/feed/collect 与
 * craft/craft-collect。田园主分区仍在 _inner.tsx + _inner-sections.tsx。
 */
import { useCallback, useEffect, useState } from "react";
import { PANEL_LG } from "@/lib/ui-classes";
import { api, ApiError } from "@/lib/api-client";
import { useI18n } from "@/i18n/client";

interface RanchAnimal {
  key: string;
  name: string;
  icon: string;
  price: number;
  yield: number;
  cycle_mins: number;
  owned: boolean;
  ready_at: string | null;
}
interface RanchRecipe {
  key: string;
  name: string;
  icon: string;
  in: number;
  out: number;
  mins: number;
  crafting: boolean;
  ready_at: string | null;
}
interface RanchData {
  animals: RanchAnimal[];
  recipes: RanchRecipe[];
}

function leftText(iso: string | null): string {
  if (!iso) return "…";
  const ms = new Date(iso).getTime() - Date.now();
  if (ms <= 0) return "";
  const m = Math.ceil(ms / 60000);
  return m >= 60 ? `${Math.floor(m / 60)}h${m % 60}m` : `${m}m`;
}

/** 牧场：动物栏（未购=买 / 已购=投喂+收集） */
export function RanchSection({
  tf,
  onErr,
}: {
  tf: Record<string, string>;
  onErr: (m: string | null) => void;
}) {
  const { dict, currency } = useI18n();
  const [data, setData] = useState<RanchData | null>(null);
  const [busy, setBusy] = useState<string | null>(null);
  const [msg, setMsg] = useState("");

  const load = useCallback(async () => {
    try {
      setData(await api.get<RanchData>("/api/v1/farm/ranch"));
    } catch {
      /* 模块未开/未配置时整段隐藏 */
    }
  }, []);
  useEffect(() => {
    void load();
  }, [load]);

  const act = useCallback(
    async (path: string, body: Record<string, unknown>, okText: string) => {
      setBusy(path + body.animal);
      onErr(null);
      try {
        await api.post(`/api/v1${path}`, body);
        setMsg(okText);
        await load();
      } catch (e) {
        onErr(
          e instanceof ApiError
            ? (dict.errors[e.code] ?? e.message)
            : dict.common.networkError,
        );
      } finally {
        setBusy(null);
      }
    },
    [load, onErr, dict],
  );

  if (!data || data.animals.length === 0) return null;
  return (
    <section className={PANEL_LG}>
      <h2 className="mb-3 font-display text-base">{tf.ranchTitle}</h2>
      <div className="grid gap-3 sm:grid-cols-2 lg:grid-cols-3">
        {data.animals.map((a) => {
          const left = a.owned ? leftText(a.ready_at) : "";
          const ready = a.owned && left === "";
          return (
            <div
              key={a.key}
              className="flex items-center gap-3 rounded-[14px] border border-line bg-[var(--surface-card)] p-3 shadow-[var(--shadow-card)]"
            >
              <span className="grid h-12 w-12 shrink-0 place-items-center rounded-full bg-[var(--surface-sunken)] text-2xl">
                {a.icon}
              </span>
              <div className="min-w-0 flex-1">
                <p className="truncate text-sm font-bold">{a.name}</p>
                <p className="num text-[11px] text-sub">
                  {tf.ranchCycle.replace("{n}", String(a.cycle_mins))} ·+
                  {a.yield} {currency}
                </p>
              </div>
              {a.owned ? (
                <div className="flex shrink-0 flex-col gap-1">
                  {ready ? (
                    <button
                      type="button"
                      disabled={busy !== null}
                      onClick={() =>
                        void act(
                          "/farm/ranch/collect",
                          { animal: a.key },
                          tf.ranchGot.replace("{n}", String(a.yield)),
                        )
                      }
                      className="min-h-[30px] rounded-full bg-mint px-3 text-[11px] font-bold text-white active:scale-[0.97] disabled:opacity-50"
                    >
                      {tf.ranchCollect}
                    </button>
                  ) : (
                    <button
                      type="button"
                      disabled={busy !== null}
                      onClick={() =>
                        void act(
                          "/farm/ranch/feed",
                          { animal: a.key },
                          tf.ranchFed,
                        )
                      }
                      className="min-h-[30px] rounded-full bg-sky-soft px-3 text-[11px] font-bold text-ink active:scale-[0.97] disabled:opacity-50"
                    >
                      {tf.ranchFeed} · {left}
                    </button>
                  )}
                </div>
              ) : (
                <button
                  type="button"
                  disabled={busy !== null}
                  onClick={() =>
                    void act(
                      "/farm/ranch/buy",
                      { animal: a.key },
                      tf.ranchBought.replace("{n}", a.name),
                    )
                  }
                  className="min-h-[30px] shrink-0 rounded-full bg-sun px-3 text-[11px] font-bold text-ink active:scale-[0.97] disabled:opacity-50"
                >
                  {tf.ranchBuy} · {a.price}
                </button>
              )}
            </div>
          );
        })}
      </div>
      {msg && <p className="mt-2 text-xs font-bold text-mint">{msg}</p>}
    </section>
  );
}

/** 加工坊：配方进度条 + 开始制作 / 收取成品 */
export function CraftSection({
  tf,
  onErr,
}: {
  tf: Record<string, string>;
  onErr: (m: string | null) => void;
}) {
  const { dict } = useI18n();
  const [data, setData] = useState<RanchData | null>(null);
  const [busy, setBusy] = useState<string | null>(null);
  const [, setNow] = useState(0);

  const load = useCallback(async () => {
    try {
      setData(await api.get<RanchData>("/api/v1/farm/ranch"));
    } catch {
      /* ignore */
    }
  }, []);
  useEffect(() => {
    void load();
    const id = window.setInterval(() => setNow(Date.now()), 1000);
    return () => window.clearInterval(id);
  }, [load]);

  const act = useCallback(
    async (path: string, body: Record<string, unknown>, ok: () => void) => {
      setBusy(path + body.recipe);
      onErr(null);
      try {
        await api.post(`/api/v1${path}`, body);
        ok();
        await load();
      } catch (e) {
        onErr(
          e instanceof ApiError
            ? (dict.errors[e.code] ?? e.message)
            : dict.common.networkError,
        );
      } finally {
        setBusy(null);
      }
    },
    [load, onErr, dict],
  );

  if (!data || data.recipes.length === 0) return null;
  return (
    <section className={PANEL_LG}>
      <h2 className="mb-3 font-display text-base">{tf.craftTitle}</h2>
      <div className="grid gap-3 sm:grid-cols-2 lg:grid-cols-3">
        {data.recipes.map((r) => {
          const left = r.crafting ? leftText(r.ready_at) : "";
          const ready = r.crafting && left === "";
          const pct = r.crafting
            ? Math.max(
                0,
                Math.min(
                  100,
                  100 -
                    (new Date(r.ready_at!).getTime() - Date.now()) /
                      (r.mins * 600),
                ),
              )
            : 0;
          return (
            <div
              key={r.key}
              className="flex flex-col gap-2 rounded-[14px] border border-line bg-[var(--surface-card)] p-3 shadow-[var(--shadow-card)]"
            >
              <div className="flex items-center gap-3">
                <span className="grid h-11 w-11 shrink-0 place-items-center rounded-full bg-[var(--surface-sunken)] text-xl">
                  {r.icon}
                </span>
                <div className="min-w-0 flex-1">
                  <p className="truncate text-sm font-bold">{r.name}</p>
                  <p className="num text-[11px] text-sub">
                    {r.in} → +{r.out} ·{" "}
                    {tf.craftMins.replace("{n}", String(r.mins))}
                  </p>
                </div>
              </div>
              <div className="h-1.5 overflow-hidden rounded-full bg-[var(--surface-sunken)]">
                <i
                  className={`block h-full rounded-full ${
                    ready ? "bg-mint" : "bg-sun"
                  }`}
                  style={{ width: `${ready ? 100 : pct}%` }}
                />
              </div>
              {r.crafting ? (
                ready ? (
                  <button
                    type="button"
                    disabled={busy !== null}
                    onClick={() =>
                      void act(
                        "/farm/craft/collect",
                        { recipe: r.key },
                        () => {},
                      )
                    }
                    className="min-h-[32px] rounded-full bg-mint text-[11px] font-bold text-white active:scale-[0.97] disabled:opacity-50"
                  >
                    {tf.craftDone}
                  </button>
                ) : (
                  <p className="num text-center text-[11px] font-bold text-sub">
                    {tf.craftBusy} · {left}
                  </p>
                )
              ) : (
                <button
                  type="button"
                  disabled={busy !== null}
                  onClick={() =>
                    void act("/farm/craft", { recipe: r.key }, () => {})
                  }
                  className="min-h-[32px] rounded-full bg-sun text-[11px] font-bold text-ink active:scale-[0.97] disabled:opacity-50"
                >
                  {tf.craftStart} · {r.in}
                </button>
              )}
            </div>
          );
        })}
      </div>
    </section>
  );
}

interface FarmCollection {
  crops: {
    got: number;
    total: number;
    items: { id: number; name: string; active: boolean; lit: boolean }[];
  };
  animals: {
    got: number;
    total: number;
    items: { key: string; name: string; icon: string; lit: boolean }[];
  };
}

/** 图鉴：作物（种过=点亮）+ 动物（养过=点亮）+ 全收录进度 */
export function FarmAlbumSection({ tf }: { tf: Record<string, string> }) {
  const [data, setData] = useState<FarmCollection | null>(null);
  const [ranch, setRanch] = useState<RanchData | null>(null);

  useEffect(() => {
    void api
      .get<FarmCollection>("/api/v1/farm/collection")
      .then(setData)
      .catch(() => {});
    void api
      .get<RanchData>("/api/v1/farm/ranch")
      .then(setRanch)
      .catch(() => {});
  }, []);

  if (!data) return null;
  const animals = ranch?.animals ?? [];
  const animalItems = data.animals.total
    ? data.animals.items
    : animals.map((a) => ({
        key: a.key,
        name: a.name,
        icon: a.icon,
        lit: a.owned,
      }));
  const animalGot = animalItems.filter((a) => a.lit).length;
  return (
    <section className={PANEL_LG}>
      <h2 className="mb-3 font-display text-base">{tf.albumTitle}</h2>
      <div className="mb-3 flex flex-wrap gap-x-6 gap-y-1 text-sm">
        <span>
          {tf.albumCrops}{" "}
          <b className="num">
            {data.crops.got}/{data.crops.total}
          </b>
        </span>
        {animalItems.length > 0 && (
          <span>
            {tf.albumAnimals}{" "}
            <b className="num">
              {animalGot}/{animalItems.length}
            </b>
          </span>
        )}
      </div>
      <div className="grid grid-cols-4 gap-2 sm:grid-cols-6 lg:grid-cols-8">
        {data.crops.items.map((c) => (
          <div
            key={c.id}
            className={`grid aspect-square place-items-center rounded-[12px] border p-1 text-center ${
              c.lit
                ? "border-sun bg-sun-soft"
                : "border-dashed border-[var(--border-deep)] bg-[var(--surface-raised)] opacity-60"
            }`}
            title={c.name}
          >
            <span className="text-[11px] font-bold leading-tight">
              {c.lit ? c.name : "???"}
            </span>
          </div>
        ))}
        {animalItems.map((a) => (
          <div
            key={a.key}
            className={`grid aspect-square place-items-center rounded-[12px] border p-1 text-center ${
              a.lit
                ? "border-mint bg-[var(--surface-card)]"
                : "border-dashed border-[var(--border-deep)] bg-[var(--surface-raised)] opacity-60"
            }`}
            title={a.name}
          >
            <span className="text-xl">{a.lit ? a.icon : "🔒"}</span>
          </div>
        ))}
      </div>
    </section>
  );
}
