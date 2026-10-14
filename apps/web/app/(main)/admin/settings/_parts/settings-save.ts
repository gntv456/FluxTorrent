"use client";

/**
 * 站点设定保存域（从 app/(main)/admin/settings/settings-client.tsx 按域拆出）：
 * useSettingsSave 未保存计数、Ctrl/Cmd+S 快捷键、离开未保存提醒、
 * 按分区归并提交与错误/提示回填。i18n 文案键（invalidField / savedToast）
 * 与 dict 由调用方传入。保存结果经全局 toast 协议反馈（DS-PLAN-01
 * W-10 试点：原本地 SettingsToast 条收编为 toast.success/error）。
 */

import { useCallback, useEffect, useMemo, useRef, useState } from "react";
import { api, ApiError } from "@/lib/api-client";
import { apiErrorMessage } from "@/i18n/client";
import { fmt } from "@/i18n/config";
import { toast } from "@/components/ui/toast";
import type { SaveResult } from "./settings-types";

export function useSettingsSave({
  edited,
  original,
  fieldGroup,
  fieldUpdatedAt,
  editable,
  dict,
  invalidMsg,
  conflictMsg,
  savedMsg,
  savedDesc,
  reload,
}: {
  edited: Record<string, string>;
  original: Record<string, string>;
  fieldGroup: Record<string, string>;
  /** name -> 页面加载时的 updated_at（ISO），乐观并发随提交回传 */
  fieldUpdatedAt: Record<string, string>;
  editable: boolean;
  dict: Parameters<typeof apiErrorMessage>[0];
  invalidMsg: string;
  conflictMsg: string;
  savedMsg: string;
  /** 成功提示第二行（缓存提示等），可选 */
  savedDesc?: string;
  reload: () => Promise<void>;
}) {
  const [saving, setSaving] = useState(false);
  const [errors, setErrors] = useState<Record<string, string>>({});
  const [lastSaved, setLastSaved] = useState<Date | null>(null);

  // 未保存计数
  const dirty = useMemo(
    () =>
      Object.entries(edited)
        .filter(([k, v]) => v !== (original[k] ?? ""))
        .map(([k]) => k),
    [edited, original],
  );
  const dirtyRef = useRef(0);
  dirtyRef.current = dirty.length;

  // Ctrl/Cmd+S 保存
  const saveAllRef = useRef<() => void>(() => {});
  useEffect(() => {
    function onKey(e: KeyboardEvent) {
      if ((e.ctrlKey || e.metaKey) && e.key.toLowerCase() === "s") {
        e.preventDefault();
        saveAllRef.current();
      }
    }
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
  }, []);

  // 离开页面未保存提醒
  useEffect(() => {
    function onBeforeUnload(e: BeforeUnloadEvent) {
      if (dirtyRef.current === 0) return;
      e.preventDefault();
      e.returnValue = "";
    }
    window.addEventListener("beforeunload", onBeforeUnload);
    return () => window.removeEventListener("beforeunload", onBeforeUnload);
  }, []);

  const saveAll = useCallback(async () => {
    if (dirty.length === 0 || saving || !editable) return;
    // 按分区归并（后端接口以分区为单位）
    const byGroup = new Map<string, Record<string, string>>();
    // 乐观并发：被修改键的 updated_at（页面加载时带回）随组提交
    const expectedByGroup = new Map<string, Record<string, string>>();
    for (const name of dirty) {
      const g = fieldGroup[name] ?? "misc";
      const bucket = byGroup.get(g) ?? {};
      bucket[name] = edited[name];
      byGroup.set(g, bucket);
      const at = fieldUpdatedAt[name];
      if (at) {
        const eb = expectedByGroup.get(g) ?? {};
        eb[name] = at;
        expectedByGroup.set(g, eb);
      }
    }
    setSaving(true);
    setErrors({});
    let saved = 0;
    const effects: string[] = [];
    const errs: Record<string, string> = {};
    const conflictKeys: string[] = [];
    let conflictRid: string | undefined;
    try {
      for (const [group, values] of byGroup) {
        const expected = expectedByGroup.get(group);
        try {
          const r = await api.put<SaveResult>("/api/v1/admin/settings/groups", {
            group,
            values,
            ...(expected ? { expected } : {}),
          });
          saved += r.saved;
          effects.push(...r.effects);
        } catch (e) {
          if (
            e instanceof ApiError &&
            e.code === 1002 &&
            Array.isArray(e.data)
          ) {
            for (const item of e.data as { field: string; error: string }[]) {
              errs[item.field] = item.error;
            }
          } else if (
            e instanceof ApiError &&
            e.code === 5003 &&
            Array.isArray(e.data)
          ) {
            // 版本冲突（5003）：data 是冲突键列表，整批提示刷新；
            // request_id 一并进 toast（失败可诊断，报障可直接定位）
            conflictKeys.push(
              ...(e.data as string[]).filter((k) => k in values),
            );
            conflictRid = e.requestId;
          } else {
            throw e;
          }
        }
      }
      if (conflictKeys.length > 0) {
        toast.error(fmt(conflictMsg, { names: conflictKeys.join("、") }), {
          requestId: conflictRid,
          action: {
            label: dict.common.retry,
            onClick: () => void reload(),
          },
        });
      } else if (Object.keys(errs).length > 0) {
        setErrors(errs);
        toast.error(fmt(invalidMsg, { n: Object.keys(errs).length }));
      } else {
        toast.success(fmt(savedMsg, { n: saved }), {
          desc: [savedDesc, ...effects].filter(Boolean).join("；") || undefined,
        });
        setLastSaved(new Date());
        await reload();
      }
    } catch (e) {
      const err = e instanceof ApiError ? e : undefined;
      toast.error(apiErrorMessage(dict, e), {
        requestId: err?.requestId,
      });
    } finally {
      setSaving(false);
    }
  }, [
    dirty,
    saving,
    editable,
    fieldGroup,
    fieldUpdatedAt,
    edited,
    dict,
    invalidMsg,
    conflictMsg,
    savedMsg,
    savedDesc,
    reload,
  ]);

  saveAllRef.current = saveAll;

  return { saving, errors, setErrors, lastSaved, dirty, saveAll };
}
