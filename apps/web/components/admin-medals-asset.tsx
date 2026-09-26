"use client";

import { useRef, useState } from "react";
import { BTN_XS_GHOST } from "@/lib/ui-classes";
import { useI18n } from "@/i18n/client";
import type { MedalRow } from "./admin-medals-shared";
import { MedalImagePreview } from "./admin-medals-rarity";

/** 上传按钮：sky 描边小胶囊（与「清除图片」同级） */
const UPLOAD_BTN_CLS =
  "min-h-[32px] rounded-full border border-sky px-3 text-xs font-bold" +
  " text-sky disabled:opacity-40";

/** 资源条容器：底部分隔线与表单主体隔开 */
const ASSET_BAR_CLS =
  "mb-3 flex flex-wrap items-center gap-3 border-b border-line pb-3";

/** 勋章图片资源条（从 admin-medals-form.tsx 拆出）：URL 输入 + 站内图床上传
 *  + 预览 + 清除。asset_ref 与头像 / 头像框同口径存 URL（不存文件）；上传
 *  走既有 POST /api/v1/attachments（multipart），成功后回填返回的站内地址。 */
export function MedalAssetBar({
  edit,
  setEdit,
  flash,
  inp,
}: {
  edit: { id: number | null; f: Partial<MedalRow> };
  setEdit: React.Dispatch<
    React.SetStateAction<{ id: number | null; f: Partial<MedalRow> }>
  >;
  flash: (m: string) => void;
  inp: string;
}) {
  const { dict } = useI18n();
  const at = dict.adminMedals;
  const pickRef = useRef<HTMLInputElement | null>(null);
  const [busy, setBusy] = useState(false);
  const setAsset = (v: string | null) =>
    setEdit((prev) => ({ ...prev, f: { ...prev.f, asset_ref: v } }));

  async function upload(file: File) {
    setBusy(true);
    try {
      const fd = new FormData();
      fd.append("file", file);
      // 凭证由 HttpOnly flux_token cookie 自动携带（同源 rewrites 转发）
      const res = await fetch("/api/v1/attachments", {
        method: "POST",
        body: fd,
      });
      const j = (await res.json()) as {
        data?: { url?: string };
        message?: string;
      };
      if (!res.ok || !j.data?.url) throw new Error(j.message ?? "");
      setAsset(j.data.url);
      flash(at.uploadOk);
    } catch (e) {
      flash((e instanceof Error && e.message) || at.opFail);
    } finally {
      setBusy(false);
      // 清空 value：同一个文件连选两次也要能再次触发 change
      if (pickRef.current) pickRef.current.value = "";
    }
  }

  return (
    <div className={ASSET_BAR_CLS}>
      <label className="flex flex-col gap-1 text-xs">
        {at.fAsset}
        <input
          value={edit.f.asset_ref ?? ""}
          onChange={(e) => setAsset(e.target.value || null)}
          placeholder="https://…/medal.png"
          className={`${inp} w-80`}
        />
      </label>
      <button
        type="button"
        className={UPLOAD_BTN_CLS}
        disabled={busy}
        onClick={() => pickRef.current?.click()}
      >
        {busy ? at.uploading : at.uploadImage}
      </button>
      <input
        ref={pickRef}
        type="file"
        accept="image/*"
        className="hidden"
        onChange={(e) => {
          const f = e.target.files?.[0];
          if (f) upload(f);
        }}
      />
      <MedalImagePreview src={edit.f.asset_ref} />
      {edit.f.asset_ref ? (
        <button
          type="button"
          className={BTN_XS_GHOST}
          onClick={() => setAsset(null)}
        >
          {at.clearAsset}
        </button>
      ) : null}
    </div>
  );
}
