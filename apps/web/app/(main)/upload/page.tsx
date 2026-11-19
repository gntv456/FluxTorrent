/* 发布页窄屏样式（路由级加载，不回灌 pages.css） */
import "@/app/styles/upload-mobile.css";
import "@/app/styles/ux-product.css";
import { UploadForm } from "@/components/upload-form";
import { getDict } from "@/i18n/server";
import { PageHeader } from "@/components/ui/page-header";
import { PublishChecklist } from "@/components/publish-checklist";

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
      <PublishChecklist dict={dict} />
      <UploadForm initialName={initialName} />
    </div>
  );
}
