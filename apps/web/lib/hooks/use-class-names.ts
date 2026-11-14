"use client";

import { useEffect, useState } from "react";
import { api } from "@/lib/api-client";

/**
 * 站点等级名的唯一前端口径（2026-11-14 硬编码清偿）：
 *
 * 此前批量发放 / 用户详情调级 / 后台页头徽章三处读 i18n
 * `admin.classList`——三语字典各写死一份「种子/新芽/…」，
 * 站长在后台改了 user_classes.name 后这三处仍显示旧名
 * （公开等级页 /classes 早已改读 DB，同屏两个名字）。
 *
 * 真名单来自 GET /api/v1/classes（公开等级页同款端点：登录即可看，
 * classes=成长线+贵宾档，staff_classes=职务档 90+），拉一次模块级缓存
 * 全站共享；请求失败（未登录/网络）回落空表，调用方自选兜底文案。
 * 返回形状与原 i18n classList 相同的 [id, name][]，消费点零逻辑改动。
 */

export type ClassEntry = [number, string];

let cache: ClassEntry[] | null = null;
let inflight: Promise<ClassEntry[]> | null = null;

async function fetchClassList(): Promise<ClassEntry[]> {
  const r = await api.get<{
    classes: { id: number; name: string }[];
    staff_classes: { id: number; name: string }[];
  }>("/api/v1/classes");
  return [
    ...(r.classes ?? []),
    ...(r.staff_classes ?? []),
  ].map((c) => [c.id, c.name] as ClassEntry);
}

/** 等级名清单（[id, name][]，含职务档）：SSR/失败时为空表，挂载后填充。 */
export function useClassNames(): ClassEntry[] {
  const [list, setList] = useState<ClassEntry[]>(cache ?? []);
  useEffect(() => {
    if (cache) return;
    if (!inflight) {
      inflight = fetchClassList()
        .then((rows) => {
          cache = rows;
          return rows;
        })
        .catch(() => {
          inflight = null;
          return [] as ClassEntry[];
        });
    }
    let alive = true;
    inflight.then((rows) => {
      if (alive && rows.length) setList(rows);
    });
    return () => {
      alive = false;
    };
  }, []);
  return list;
}

/** 按档位 id 取名（找不到返回 null，调用方兜底）。 */
export function classNameOf(
  list: ClassEntry[],
  id: number | undefined | null,
): string | null {
  if (id === undefined || id === null) return null;
  return list.find(([cid]) => cid === id)?.[1] ?? null;
}
