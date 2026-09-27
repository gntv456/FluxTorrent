"use client";

import { useEffect, useState } from "react";
import { api } from "@/lib/api-client";
import { useI18n } from "@/i18n/client";

/** P2 触点 #2：等级要求公开页（0226，UNIT3D /groups/requirements 口径）。
 *  消费 GET /api/v1/classes——成长线门槛四维 + 特权名；登录可看。 */

interface ClassesData {
  me_class: number;
  classes: {
    id: number;
    name: string;
    min_uploaded: number;
    min_download_count: number;
    min_seed_hours: number;
    min_account_age_days: number;
    demotable: boolean;
  }[];
  staff_classes: { id: number; name: string }[];
}

function fmtGB(n: number): string {
  if (n <= 0) return "—";
  const gb = n / 1024 / 1024 / 1024;
  if (gb >= 1024) return `${(gb / 1024).toFixed(0)} TB`;
  return `${gb >= 100 ? gb.toFixed(0) : gb.toFixed(1)} GB`;
}

export default function ClassesPage() {
  const { dict } = useI18n();
  void dict;
  const [d, setD] = useState<ClassesData | null>(null);
  const [err, setErr] = useState("");

  useEffect(() => {
    api
      .get<ClassesData>("/api/v1/classes")
      .then(setD)
      .catch((e) => setErr(String(e)));
  }, []);

  if (err) return <p className="py-8 text-center text-danger">{err}</p>;
  if (!d) return <p className="py-8 text-center text-sub">…</p>;

  return (
    <div className="mx-auto max-w-4xl px-4 py-8">
      <h1 className="font-display text-xl">等级要求</h1>
      <p className="mt-1 text-xs text-sub">
        全部达标自动晋升（分钟级生效，无需申请）；「可降级」的等级跌破门槛会自动降回。
      </p>
      <div className="baozi-panel mt-4 overflow-x-auto">
        <table className="w-full text-sm">
          <thead>
            <tr className="border-b border-line text-left text-xs text-sub">
              <th className="px-3 py-2">等级</th>
              <th className="px-3 py-2">上传量</th>
              <th className="px-3 py-2">完成数</th>
              <th className="px-3 py-2">做种时长</th>
              <th className="px-3 py-2">账龄</th>
              <th className="px-3 py-2">降级</th>
            </tr>
          </thead>
          <tbody>
            {d.classes.map((c) => (
              <tr
                key={c.id}
                className={`border-b border-line/50 ${
                  c.id === d.me_class ? "bg-accent/10 font-medium" : ""
                }`}
              >
                <td className="px-3 py-2">
                  {c.name}
                  {c.id === d.me_class && (
                    <span className="ml-2 text-xs text-accent">当前</span>
                  )}
                </td>
                <td className="px-3 py-2 font-mono">
                  {fmtGB(c.min_uploaded)}
                </td>
                <td className="px-3 py-2 font-mono">
                  {c.min_download_count > 0
                    ? c.min_download_count.toLocaleString()
                    : "—"}
                </td>
                <td className="px-3 py-2 font-mono">
                  {c.min_seed_hours > 0
                    ? `${c.min_seed_hours.toLocaleString()} h`
                    : "—"}
                </td>
                <td className="px-3 py-2 font-mono">
                  {c.min_account_age_days > 0
                    ? `${c.min_account_age_days.toLocaleString()} 天`
                    : "—"}
                </td>
                <td className="px-3 py-2 text-xs">
                  {c.demotable ? "可降级" : "—"}
                </td>
              </tr>
            ))}
          </tbody>
        </table>
      </div>
      {d.staff_classes.length > 0 && (
        <p className="mt-3 text-xs text-sub">
          职务等级（站长任命，不在自动晋升线内）：
          {d.staff_classes.map((s) => s.name).join(" / ")}
        </p>
      )}
    </div>
  );
}
