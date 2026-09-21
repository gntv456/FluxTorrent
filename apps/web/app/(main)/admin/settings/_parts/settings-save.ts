"use client";

/**
 * 站点设定保存域（从 app/(main)/admin/settings/settings-client.tsx 按域拆出）：
 * useSettingsSave 未保存计数、Ctrl/Cmd+S 快捷键、离开未保存提醒、
 * 按分区归并提交与错误/提示回填。i18n 文案键（invalidField / savedToast）
 * 与 dict 由调用方传入。保存结果经回调交还主组件。
 */

import { useCallback, useEffect, useMemo, useRef, useState } from "react";
import { api, ApiError } from "@/lib/api-client";
import { apiErrorMessage } from "@/i18n/client";
import { fmt } from "@/i18n/config";
import type { SaveResult } from "./settings-types";
import type { SettingsToast } from "./settings-io";

export function useSettingsSave({
  edited,
  original,
  fieldGroup,
  editable,
  dict,
  invalidMsg,
  savedMsg,
  reload,
  onToast,
}: {
  edited: Record<string, string>;
  original: Record<string, string>;
  fieldGroup: Record<string, string>;
  editable: boolean;
  dict: Parameters<typeof apiErrorMessage>[0];
  invalidMsg: string;
  savedMsg: string;
  reload: () => Promise<void>;
  onToast: (t: SettingsToast | null) => void;
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
    for (const name of dirty) {
      const g = fieldGroup[name] ?? "misc";
      const bucket = byGroup.get(g) ?? {};
      bucket[name] = edited[name];
      byGroup.set(g, bucket);
    }
    setSaving(true);
    setErrors({});
    let saved = 0;
    const effects: string[] = [];
    const errs: Record<string, string> = {};
    try {
      for (const [group, values] of byGroup) {
        try {
          const r = await api.put<SaveResult>("/api/v1/admin/settings/groups", {
            group,
            values,
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
          } else {
            throw e;
          }
        }
      }
      if (Object.keys(errs).length > 0) {
        setErrors(errs);
        onToast({
          ok: false,
          text: fmt(invalidMsg, { n: Object.keys(errs).length }),
        });
      } else {
        onToast({ ok: true, text: fmt(savedMsg, { n: saved }), effects });
        setLastSaved(new Date());
        await reload();
      }
    } catch (e) {
      onToast({ ok: false, text: apiErrorMessage(dict, e) });
    } finally {
      setSaving(false);
    }
  }, [
    dirty,
    saving,
    editable,
    fieldGroup,
    edited,
    dict,
    invalidMsg,
    savedMsg,
    reload,
    onToast,
  ]);

  saveAllRef.current = saveAll;

  return { saving, errors, setErrors, lastSaved, dirty, saveAll };
}
