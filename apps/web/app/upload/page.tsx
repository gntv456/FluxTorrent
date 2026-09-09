import { UploadForm } from "@/components/upload-form";

export default function UploadPage() {
  return (
    <div className="flex flex-col gap-4">
      <div className="flex flex-wrap items-baseline gap-2">
        <h1 className="font-display text-2xl">发布资源</h1>
        <span className="text-sm text-sub">发布进入待审核，通过后全站可见</span>
      </div>
      <UploadForm />
    </div>
  );
}
