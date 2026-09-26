"use client";

import { useState } from "react";
import { api, ApiError } from "@/lib/api-client";
import { useI18n } from "@/i18n/client";

/** 字幕元数据编辑弹层（0150 缺口2：替换只改标题的 prompt）。
 *  字段与 PATCH /subtitles/{id} 对齐：标题/语言/FPS/来源/制作/校对/
 *  署名/AI 标记（verified 仍属 staff 专属，不在此）。 */

export interface SubtitleEditInitial {
  title: string;
  fps: string;
  source: string;
  producer: string;
  proofreader: string;
  author_name: string;
  machine_translated: boolean;
  lang: string;
}

export function SubtitleEditDialog({
  sid,
  initial,
  onClose,
  onSaved,
  langs,
}: {
  sid: number;
  initial: SubtitleEditInitial;
  onClose: () => void;
  onSaved: () => void;
  langs?: { code: string; name: string; flag: string }[];
}) {
  const { dict } = useI18n();
  const t = dict.subtitles;
  const [f, setF] = useState<SubtitleEditInitial>(initial);
  const [msg, setMsg] = useState<string | null>(null);
  const [busy, setBusy] = useState(false);

  async function save() {
    if (!f.title.trim()) {
      setMsg(t.titleLabel);
      return;
    }
    setBusy(true);
    setMsg(null);
    try {
      await api.patch(`/api/v1/subtitles/${sid}`, {
        title: f.title.trim(),
        lang: f.lang || undefined,
        fps: f.fps ? Number(f.fps) : undefined,
        source: f.source || undefined,
        producer: f.producer || undefined,
        proofreader: f.proofreader || undefined,
        author_name: f.author_name || undefined,
        machine_translated: f.machine_translated,
      });
      onSaved();
    } catch (e) {
      setMsg(e instanceof ApiError ? e.message : dict.common.networkError);
    } finally {
      setBusy(false);
    }
  }

  const row = (label: string, node: React.ReactNode) => (
    <tr>
      <td className="rowhead w-28">{label}</td>
      <td>{node}</td>
    </tr>
  );
  const inp = (
    v: string,
    set: (s: string) => void,
    ph: string,
    cls: string,
  ) => (
    <input
      type="text"
      className={cls}
      value={v}
      placeholder={ph}
      onChange={(e) => set(e.target.value)}
    />
  );

  return (
    <div
      className="fixed inset-0 z-50 flex items-center
        justify-center bg-black/50"
      onClick={onClose}
    >
      <div
        className="max-h-[85vh] w-[min(640px,92vw)]
          overflow-auto rounded border border-line
          bg-[var(--surface)] p-4"
        onClick={(e) => e.stopPropagation()}
      >
        <h2 className="mb-3 text-base font-bold">{t.editLabel}</h2>
        <div className="baozi-wide-table-scroll">
        <table className="nexus-table nexus-form">
          <tbody>
            {row(
              t.titleLabel,
              inp(
                f.title,
                (s) => setF({ ...f, title: s }),
                "",
                "w-full",
              ),
            )}
            {langs && langs.length > 0 && (
              <tr>
                <td className="rowhead w-28">{t.lang}</td>
                <td>
                  <select
                    value={f.lang}
                    onChange={(e) => setF({ ...f, lang: e.target.value })}
                  >
                    <option value="">{t.langSelect}</option>
                    {langs.map((l) => (
                      <option key={l.code} value={l.code}>
                        {l.flag} {l.name}
                      </option>
                    ))}
                  </select>
                </td>
              </tr>
            )}
            {row(
              t.fpsLabel,
              inp(
                f.fps,
                (s) => setF({ ...f, fps: s.replace(/[^\d.]/g, "") }),
                "23.976",
                "w-24",
              ),
            )}
            {row(
              t.sourceLabel,
              inp(f.source, (s) => setF({ ...f, source: s }), "", "w-64"),
            )}
            {row(
              t.producerLabel ?? "Producer",
              inp(f.producer, (s) => setF({ ...f, producer: s }), "", "w-64"),
            )}
            {row(
              t.proofreaderLabel ?? "Proofreader",
              inp(
                f.proofreader,
                (s) => setF({ ...f, proofreader: s }),
                "",
                "w-64",
              ),
            )}
            {row(
              t.authorLabel,
              inp(
                f.author_name,
                (s) => setF({ ...f, author_name: s }),
                "",
                "w-64",
              ),
            )}
            {row(
              t.aiLabel,
              <label className="flex items-center gap-1">
                <input
                  type="checkbox"
                  checked={f.machine_translated}
                  onChange={(e) =>
                    setF({ ...f, machine_translated: e.target.checked })
                  }
                />
                {t.aiNote}
              </label>,
            )}
          </tbody>
        </table>
        </div>
        {msg && <p className="mt-2 text-sm text-coral">{msg}</p>}
        <div className="mt-3 flex justify-end gap-2">
          <button type="button" className="btn2" onClick={onClose}>
            {dict.common.cancel}
          </button>
          <button
            type="button"
            className="btn"
            disabled={busy}
            onClick={() => void save()}
          >
            {t.saveLabel ?? "Save"}
          </button>
        </div>
      </div>
    </div>
  );
}
