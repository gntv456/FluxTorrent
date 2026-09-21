"use client";

import { useI18n } from "@/i18n/client";

import { useCallback, useEffect, useState } from "react";
import { api, ApiError } from "@/lib/api-client";

/** 第八轮 P3-13 + 0106 结构化改版：考核岗位类型配置
 *  指标门槛不再手写 JSON —— 下拉选指标 + 填数值（可多行 AND 组合）；
 *  加成规则也不再写代码：选档位数（每 N 个达标月）和加成百分比。
 *  底层仍存 min_requirements / bonus_rules JSONB，兼容旧数据。 */

interface JixiaoTypeRow {
  id: number;
  name: string;
  metrics: Record<string, unknown>;
  base_pay: number;
  min_requirements: Record<string, unknown>;
  bonus_rules: Record<string, unknown>;
  description?: string;
  /** 0106：本期登记人数（真实数据，旧版硬编码 "—"） */
  assigned_count?: number;
}

/** 指标目录：key 与后端 JIXIAO_METRIC_KEYS 白名单一一对应。
 *  货币名动态化：spark_delta 的标签/单位跟随站点 currency_name（默认「魔力」）。 */
const metricOptions = (
  currency: string,
): { key: string; label: string; unit: string; hint: string }[] => [
  {
    key: "uploaded",
    label: "上传增量",
    unit: "GB",
    hint: "考核期内新增上传量",
  },
  {
    key: "downloaded",
    label: "下载增量",
    unit: "GB",
    hint: "考核期内新增下载量",
  },
  {
    key: "seed_hours",
    label: "做种时长",
    unit: "小时",
    hint: "考核期内累计做种小时数",
  },
  {
    key: "avg_seed_hours",
    label: "平均做种时间",
    unit: "小时/个",
    hint: "做种时长 ÷ 期内活跃种子数",
  },
  {
    key: "seed_days",
    label: "做种天数",
    unit: "天",
    hint: "考核期内有做种活动的天数",
  },
  {
    key: "spark_delta",
    label: `${currency}增量`,
    unit: currency,
    hint: `考核期内正向${currency}流水合计`,
  },
  {
    key: "seed_points_delta",
    label: "做种积分增量",
    unit: "积分",
    hint: "1 积分 = 1 小时做种",
  },
  {
    key: "uploads",
    label: "发种增量",
    unit: "个",
    hint: "考核期内新发布种子数",
  },
  {
    key: "seed_size_tb",
    label: "发布/做种体积",
    unit: "TB",
    hint: "当前在做种总体积",
  },
  {
    key: "seeding_count",
    label: "做种数量",
    unit: "个",
    hint: "当前在做种种子数",
  },
  {
    key: "ops",
    label: "审核/操作数量",
    unit: "次",
    hint: "考核期内管理操作数（audit_log）",
  },
];

/** 上传/下载指标在后端按字节存，表单用 GB 填写；seed_size_tb 本身就是 TB */
const GB_KEYS = new Set(["uploaded", "downloaded"]);
const GB = 1024 ** 3;

/** 一行门槛 = 指标 + 数值。列表编辑 ⇄ JSON 双向转换时按 GB_KEYS 换算。 */
interface ReqRow {
  key: string;
  value: string;
}

const EMPTY_FORM = {
  name: "",
  base_pay: "0",
  description: "",
  reqs: [{ key: "seed_hours", value: "100" }] as ReqRow[],
  bonusStep: "3",
  bonusPct: "10",
};

function reqsToJson(reqs: ReqRow[]): Record<string, number> {
  const out: Record<string, number> = {};
  for (const r of reqs) {
    const v = Number(r.value);
    if (!r.key || !Number.isFinite(v) || v <= 0) continue;
    out[r.key] = GB_KEYS.has(r.key) ? Math.round(v * GB) : Math.round(v);
  }
  return out;
}

function jsonToReqs(obj: Record<string, unknown>): ReqRow[] {
  const rows = Object.entries(obj)
    .filter(([, v]) => typeof v === "number" && v > 0)
    .map(([k, v]) => ({
      key: k,
      value: String(
        GB_KEYS.has(k) ? Math.round((v as number) / GB) : (v as number),
      ),
    }));
  return rows.length ? rows : [{ key: "seed_hours", value: "100" }];
}

export function AdminExams() {
  const { currency } = useI18n();
  const METRIC_OPTIONS = metricOptions(currency);
  const [rows, setRows] = useState<JixiaoTypeRow[]>([]);
  const [edit, setEdit] = useState<{ id: number | null; f: typeof EMPTY_FORM }>(
    { id: null, f: { ...EMPTY_FORM, reqs: [...EMPTY_FORM.reqs] } },
  );
  const [msg, setMsg] = useState<string | null>(null);
  const [busy, setBusy] = useState(false);

  const flash = (m: string) => {
    setMsg(m);
    setTimeout(() => setMsg(null), 3500);
  };

  const load = useCallback(async () => {
    try {
      setRows(await api.get<JixiaoTypeRow[]>("/api/v1/admin/jixiao-types"));
    } catch (e) {
      flash(e instanceof ApiError ? e.message : "加载失败");
    }
  }, []);
  useEffect(() => {
    load();
  }, [load]);

  function startEdit(r: JixiaoTypeRow) {
    const rules = r.bonus_rules as {
      months_per_step?: number;
      percent_per_step?: number;
    };
    setEdit({
      id: r.id,
      f: {
        name: r.name,
        base_pay: String(r.base_pay),
        description: r.description ?? "",
        reqs: jsonToReqs(r.min_requirements ?? {}),
        bonusStep: String(rules.months_per_step ?? 3),
        bonusPct: String(rules.percent_per_step ?? 10),
      },
    });
  }

  async function save() {
    setBusy(true);
    try {
      const payload = {
        name: edit.f.name,
        metrics: {},
        base_pay: Number(edit.f.base_pay) || 0,
        min_requirements: reqsToJson(edit.f.reqs),
        // bonus_rules 结构化落库（当前结算走 site_settings 全局配置，
        // 此处存岗位级意图，后续按岗位覆盖时直接可用）
        bonus_rules: {
          months_per_step: Number(edit.f.bonusStep) || 3,
          percent_per_step: Number(edit.f.bonusPct) || 0,
        },
        description: edit.f.description,
      };
      if (edit.id === null)
        await api.post("/api/v1/admin/jixiao-types", payload);
      else await api.put(`/api/v1/admin/jixiao-types/${edit.id}`, payload);
      flash("已保存");
      setEdit({ id: null, f: { ...EMPTY_FORM, reqs: [...EMPTY_FORM.reqs] } });
      await load();
    } catch (e) {
      flash(e instanceof ApiError ? e.message : "操作失败");
    } finally {
      setBusy(false);
    }
  }

  const inp =
    "min-h-[40px] rounded-[var(--r-sm)] border border-line bg-cloud px-2 text-sm outline-none focus:border-sky";

  const metricLabel = (k: string) =>
    METRIC_OPTIONS.find((m) => m.key === k)?.label ?? k;
  const metricUnit = (k: string) =>
    METRIC_OPTIONS.find((m) => m.key === k)?.unit ?? "";
  const fmtReq = (k: string, v: number) =>
    GB_KEYS.has(k)
      ? `${Math.round(v / GB)} GB`
      : `${v} ${metricUnit(k)}`.trim();

  return (
    <div className="flex flex-col gap-3">
      {msg && (
        <p className="rounded-[var(--r-md)] bg-sky-soft p-3 text-sm text-ink">
          {msg}
        </p>
      )}
      <section className="baozi-panel cmgmt-form p-4">
        <h2 className="mb-2 text-base font-bold">
          {edit.id === null ? "新建考核岗位" : `编辑考核岗位 #${edit.id}`}
        </h2>
        <p className="mb-3 text-xs text-sub">
          登记入口在「绩效考核」面板批量分配，或用户详情页单人分配；这里维护岗位、指标门槛与加成。
        </p>

        <div className="flex flex-wrap items-end gap-2">
          <label className="flex flex-col gap-1 text-xs">
            岗位名
            <input
              value={edit.f.name}
              onChange={(e) =>
                setEdit({ ...edit, f: { ...edit.f, name: e.target.value } })
              }
              className={`${inp} w-36`}
            />
          </label>
          <label className="flex flex-col gap-1 text-xs">
            底薪({currency})
            <input
              type="number"
              value={edit.f.base_pay}
              onChange={(e) =>
                setEdit({ ...edit, f: { ...edit.f, base_pay: e.target.value } })
              }
              className={`${inp} w-24`}
            />
          </label>
          <label className="flex flex-col gap-1 text-xs">
            岗位说明
            <input
              value={edit.f.description}
              onChange={(e) =>
                setEdit({
                  ...edit,
                  f: { ...edit.f, description: e.target.value },
                })
              }
              placeholder="展示给成员的岗位职责（可空）"
              className={`${inp} w-56`}
            />
          </label>
        </div>

        {/* 达标门槛：结构化行（指标下拉 + 数值），多行之间是 AND */}
        <div className="mt-3">
          <p className="mb-1 text-xs font-bold">达标门槛（全部满足才算达标）</p>
          <div className="flex flex-col gap-2">
            {edit.f.reqs.map((r, i) => {
              const opt = METRIC_OPTIONS.find((m) => m.key === r.key);
              return (
                <div key={i} className="flex flex-wrap items-center gap-2">
                  <select
                    value={r.key}
                    onChange={(e) => {
                      const reqs = [...edit.f.reqs];
                      reqs[i] = { ...reqs[i], key: e.target.value };
                      setEdit({ ...edit, f: { ...edit.f, reqs } });
                    }}
                    className={`${inp} w-44`}
                  >
                    {METRIC_OPTIONS.map((m) => (
                      <option key={m.key} value={m.key}>
                        {m.label}（{m.unit}）
                      </option>
                    ))}
                  </select>
                  <input
                    type="number"
                    min="0"
                    value={r.value}
                    onChange={(e) => {
                      const reqs = [...edit.f.reqs];
                      reqs[i] = { ...reqs[i], value: e.target.value };
                      setEdit({ ...edit, f: { ...edit.f, reqs } });
                    }}
                    className={`${inp} w-24`}
                  />
                  <span className="text-xs text-sub">{metricUnit(r.key)}</span>
                  {opt && <span className="text-xs text-sub">{opt.hint}</span>}
                  <button
                    type="button"
                    className="cmgmt-act cmgmt-act--danger"
                    disabled={edit.f.reqs.length <= 1}
                    onClick={() =>
                      setEdit({
                        ...edit,
                        f: {
                          ...edit.f,
                          reqs: edit.f.reqs.filter((_, j) => j !== i),
                        },
                      })
                    }
                  >
                    删除
                  </button>
                </div>
              );
            })}
          </div>
          <button
            type="button"
            className="mt-2 min-h-[32px] rounded-full border border-line px-4 text-xs font-bold"
            onClick={() =>
              setEdit({
                ...edit,
                f: {
                  ...edit.f,
                  reqs: [...edit.f.reqs, { key: "uploads", value: "10" }],
                },
              })
            }
          >
            + 添加指标
          </button>
        </div>

        {/* 加成规则：小白化（每 N 个达标月 +M%） */}
        <div className="mt-3 flex flex-wrap items-end gap-2">
          <p className="w-full text-xs font-bold">连续达标加成</p>
          <span className="pb-2 text-xs text-sub">每</span>
          <label className="flex flex-col gap-1 text-xs">
            <span className="sr-only">档位月数</span>
            <input
              type="number"
              min="1"
              value={edit.f.bonusStep}
              onChange={(e) =>
                setEdit({
                  ...edit,
                  f: { ...edit.f, bonusStep: e.target.value },
                })
              }
              className={`${inp} w-16`}
            />
          </label>
          <span className="pb-2 text-xs text-sub">个达标月，工资加</span>
          <label className="flex flex-col gap-1 text-xs">
            <span className="sr-only">加成百分比</span>
            <input
              type="number"
              min="0"
              max="100"
              value={edit.f.bonusPct}
              onChange={(e) =>
                setEdit({ ...edit, f: { ...edit.f, bonusPct: e.target.value } })
              }
              className={`${inp} w-16`}
            />
          </label>
          <span className="pb-2 text-xs text-sub">
            %（此岗位结算即按此比例；留空或 0 则用全站默认）
          </span>
        </div>

        <div className="mt-3 flex gap-2">
          <button
            className="baozi-button"
            disabled={busy || !edit.f.name.trim()}
            onClick={save}
          >
            保存
          </button>
          {edit.id !== null && (
            <button
              className="min-h-[36px] rounded-full border border-line px-4 text-xs font-bold"
              onClick={() =>
                setEdit({
                  id: null,
                  f: { ...EMPTY_FORM, reqs: [...EMPTY_FORM.reqs] },
                })
              }
            >
              取消
            </button>
          )}
        </div>
      </section>

      <table className="nexus-table text-xs">
        <thead>
          <tr>
            <td className="colhead">ID</td>
            <td className="colhead">岗位</td>
            <td className="colhead">底薪</td>
            <td className="colhead">达标门槛</td>
            <td className="colhead">加成</td>
            <td className="colhead">登记数</td>
            <td className="colhead text-right">操作</td>
          </tr>
        </thead>
        <tbody>
          {rows.map((r) => {
            const rules = r.bonus_rules as {
              months_per_step?: number;
              percent_per_step?: number;
            };
            const reqs = Object.entries(r.min_requirements ?? {})
              .filter(([, v]) => typeof v === "number" && (v as number) > 0)
              .map(([k, v]) => `${metricLabel(k)} ${fmtReq(k, v as number)}`)
              .join(" 且 ");
            return (
              <tr key={r.id}>
                <td className="num">{r.id}</td>
                <td className="font-bold">
                  {r.name}
                  {r.description ? (
                    <span
                      className="ml-1 font-normal text-sub"
                      title={r.description}
                    >
                      ⓘ
                    </span>
                  ) : null}
                </td>
                <td className="num">{r.base_pay}</td>
                <td className="max-w-[320px] truncate">{reqs || "无门槛"}</td>
                <td className="num">
                  {rules.percent_per_step
                    ? `每${rules.months_per_step ?? 3}月+${rules.percent_per_step}%`
                    : "—"}
                </td>
                <td className="num">{r.assigned_count ?? "—"}</td>
                <td className="text-right">
                  <button className="cmgmt-act" onClick={() => startEdit(r)}>
                    编辑
                  </button>
                  <button
                    className="cmgmt-act cmgmt-act--danger"
                    disabled={busy}
                    onClick={async () => {
                      try {
                        await api.del(`/api/v1/admin/jixiao-types/${r.id}`);
                        flash("已删除");
                        await load();
                      } catch (e) {
                        flash(e instanceof ApiError ? e.message : "删除失败");
                      }
                    }}
                  >
                    删除
                  </button>
                </td>
              </tr>
            );
          })}
          {rows.length === 0 && (
            <tr>
              <td colSpan={7} className="py-6 text-center text-sub">
                暂无考核岗位
              </td>
            </tr>
          )}
        </tbody>
      </table>
    </div>
  );
}
