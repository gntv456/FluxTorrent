import Link from "next/link";
import { UploadForm } from "@/components/upload-form";
import { getDict } from "@/i18n/server";

export default async function UploadPage() {
  const { dict } = await getDict();
  return (
    <div className="flex flex-col gap-4">
      <nav className="text-sm text-sub">
        <Link href="/" className="text-sky hover:text-[var(--baozi-orange)]">
          FluxTorrent
        </Link>
        <span className="mx-1">»</span>
        <span>{dict.upload.title}</span>
      </nav>
      <h1 className="font-display text-2xl">{dict.upload.title}</h1>
      <UploadForm />
    </div>
  );
}
