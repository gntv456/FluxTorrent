import { UploadForm } from "@/components/upload-form";
import { getDict } from "@/i18n/server";

export default async function UploadPage() {
  const { dict } = await getDict();
  return (
    <div className="flex flex-col gap-4">
      <div className="flex flex-wrap items-baseline gap-2">
        <h1 className="font-display text-2xl">{dict.upload.title}</h1>
        <span className="text-sm text-sub">{dict.upload.subtitle}</span>
      </div>
      <UploadForm />
    </div>
  );
}
