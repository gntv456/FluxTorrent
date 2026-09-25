"use client";

/**
 * 论坛视频插入行（0189/0190，从 forum-composer.tsx 按域拆出，300 门禁）：
 *  - 贴链接试匹配白名单 → 预览命中名 → 插入 !video(URL)；
 *  - 或选本地 mp4/webm 上传：浏览器抽时长/封面随 meta 提交（服务端魔数兜底），
 *    成功后同样插入 !video(站内 URL)。
 */

import { useEffect, useState } from "react";
import { api } from "@/lib/api-client";
import { useI18n } from "@/i18n/client";
import { matchEmbedRule } from "@/components/forum-video-embed";
import type { EmbedRule } from "@/lib/data-forum";

const PILL_BTN =
  "rounded-full border border-line px-3 py-1 text-xs font-bold " +
  "text-sub hover:border-sky hover:text-sky";
const URL_INPUT =
  "min-w-[200px] flex-1 rounded-[var(--r-sm)] border border-line " +
  "bg-cloud px-3 py-1.5 text-sm outline-none focus:border-sky";

/** 浏览器端抽元数据（时长/分辨率/首帧封面）——不引服务端 FFmpeg。
 *  封面转 jpeg 走既有图片附件路径存 poster_sha；抽取失败不阻塞上传。 */
async function probeVideo(file: File) {
  const v = document.createElement("video");
  v.preload = "metadata";
  v.muted = true;
  const url0 = URL.createObjectURL(file);
  const meta: Record<string, unknown> = {};
  try {
    await new Promise<void>((res, rej) => {
      v.onloadedmetadata = () => res();
      v.onerror = () => rej(new Error("meta"));
      v.src = url0;
    });
    meta.duration = Math.round(v.duration * 10) / 10;
    meta.width = v.videoWidth;
    meta.height = v.videoHeight;
    v.currentTime = Math.min(1, v.duration / 2);
    await new Promise<void>((res) => {
      v.onseeked = () => res();
      v.onerror = () => res();
    });
    const canvas = document.createElement("canvas");
    canvas.width = 480;
    canvas.height = Math.round(
      (480 * v.videoHeight) / Math.max(1, v.videoWidth),
    );
    canvas.getContext("2d")?.drawImage(v, 0, 0, canvas.width, canvas.height);
    const blob: Blob | null = await new Promise((res) =>
      canvas.toBlob((b) => res(b), "image/jpeg", 0.8),
    );
    if (blob) {
      // FormData 直传（api.post 走 JSON.stringify，不适用于 multipart）
      const fd = new FormData();
      fd.append(
        "file",
        new File([blob], "poster.jpg", { type: "image/jpeg" }),
      );
      const res = await fetch("/api/v1/attachments", {
        method: "POST",
        body: fd,
      });
      const j = (await res.json()) as {
        data?: { sha256?: string };
        message?: string;
      };
      if (j.data?.sha256) meta.poster_sha = j.data.sha256;
    }
  } catch {
    /* 元数据抽取失败不阻塞上传（服务端只兜魔数） */
  } finally {
    URL.revokeObjectURL(url0);
  }
  return meta;
}

export function VideoInsertRow({
  onInsert,
}: {
  onInsert: (line: string) => void;
}) {
  const { dict } = useI18n();
  const t = dict.forums;
  const [url, setUrl] = useState("");
  const [rules, setRules] = useState<EmbedRule[] | null>(null);
  const [busy, setBusy] = useState(false);

  useEffect(() => {
    let alive = true;
    api
      .get<EmbedRule[]>("/api/v1/forums/embed-rules")
      .then((r) => {
        if (alive) setRules(r ?? []);
      })
      .catch(() => {
        if (alive) setRules([]);
      });
    return () => {
      alive = false;
    };
  }, []);

  async function upload(file: File) {
    setBusy(true);
    try {
      const meta = await probeVideo(file);
      const fd = new FormData();
      fd.append("file", file);
      fd.append("meta", JSON.stringify(meta));
      const res = await fetch("/api/v1/attachments/video", {
        method: "POST",
        body: fd,
      });
      const j = (await res.json()) as {
        data?: { url?: string };
        message?: string;
      };
      if (!res.ok || !j.data?.url) {
        throw new Error(j.message ?? dict.common.networkError);
      }
      onInsert(`!video(${j.data.url})`);
    } catch (e) {
      window.alert(
        e instanceof Error && e.message ? e.message : dict.common.networkError,
      );
    } finally {
      setBusy(false);
    }
  }

  const hit = url.trim() ? matchEmbedRule(url.trim(), rules) : null;
  return (
    <div className="flex flex-col gap-1">
      <span className="text-sm text-sub">{t.videoBtn}</span>
      <div className="flex flex-wrap items-center gap-2">
        <input
          value={url}
          onChange={(e) => setUrl(e.target.value)}
          maxLength={500}
          placeholder={t.videoPh}
          className={URL_INPUT}
        />
        <button
          type="button"
          disabled={!url.trim()}
          onClick={() => {
            onInsert(`!video(${url.trim()})`);
            setUrl("");
          }}
          className={`${PILL_BTN} disabled:opacity-40`}
        >
          {t.videoInsert}
        </button>
        <label className={`cursor-pointer ${PILL_BTN}`}>
          {busy ? t.videoUploading : t.videoUpload}
          <input
            type="file"
            accept="video/mp4,video/webm"
            className="hidden"
            disabled={busy}
            onChange={(e) => {
              const f = e.target.files?.[0];
              if (f) upload(f);
              e.target.value = "";
            }}
          />
        </label>
      </div>
      {url.trim() && (
        <span className="text-xs text-sub">
          {hit
            ? t.videoPreview.replace("{name}", hit.rule.name_zh)
            : t.videoNoHit}
        </span>
      )}
    </div>
  );
}
