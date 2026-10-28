import { requireModule } from "@/components/module-gate";
import MyExamsPage from "./_inner";

/** 考核是可选模块：关闭时页面与导航同口径，不渲染内部结构、也不发 4101 请求。 */
export default async function ExamsPageWrapper() {
  const gate = await requireModule("exams");
  if (gate) return gate;
  return <MyExamsPage />;
}
