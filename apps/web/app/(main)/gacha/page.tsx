import { requireModule } from "@/components/module-gate";
import GachaDisclosurePage from "./_inner";

/**
 * G31-A 只读公示页（方案《抽卡玩法落地方案-2026-09-27》§5 样张⑥）。
 * 匿名可读（口径与后端三端点一致）；模块关闭时 requireModule 渲染统一空态。
 */
export default async function GachaPageWrapper() {
  const gate = await requireModule("gacha");
  if (gate) return gate;
  /* 娱乐屋样图⑧（星轨卡册）落在公示页上——与 /games/* 同挂甜梦皮肤 */
  return (
    <div data-arcade="sweet">
      <GachaDisclosurePage />
    </div>
  );
}
