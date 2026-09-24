import { UploadForm } from "@/components/upload-form";
import { getDict } from "@/i18n/server";

export default async function UploadPage() {
  const { dict } = await getDict();
  return (
    <div className="flex flex-col gap-4">
      <div className="pghd">
        <div>
          <div className="pg-eyebrow">Upload</div>
          <h1 className="font-display text-2xl">{dict.upload.title}</h1>
        </div>
        <span className="sub">{dict.upload.subtitle}</span>
      </div>
      <UploadForm />
    </div>
  );
}
