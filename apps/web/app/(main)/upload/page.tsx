import { UploadForm } from "@/components/upload-form";
import { getDict } from "@/i18n/server";
import { PageHeader } from "@/components/ui/page-header";

export default async function UploadPage({
  searchParams,
}: {
  searchParams: Promise<{ group?: string | string[] }>;
}) {
  const { group } = await searchParams;
  const initialName = Array.isArray(group) ? group[0] : group;
  const { dict } = await getDict();
  return (
    <div className="flex flex-col gap-4">
      <PageHeader
        eyebrow="Upload"
        title={dict.upload.title}
        sub={dict.upload.subtitle}
      />
      <UploadForm initialName={initialName} />
    </div>
  );
}
