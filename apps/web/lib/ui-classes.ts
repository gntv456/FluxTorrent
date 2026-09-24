// 全站共享的 Tailwind 类串常量（行宽门禁友好 + 单点改样式）。
// 从 66 个组件中重复 >=3 次的 className 收编而来；新组件优先用这里的常量，
// 而不是复制粘贴长类串。命名规则：<用途>_<形态>。

/** 次要按钮：36px 胶囊描边（小号内边距 + 禁用态） */
export const BTN_SM_GHOST =
  "min-h-[36px] rounded-full border border-line px-3 disabled:opacity-40";

/** 次要按钮：36px 胶囊描边（粗体小字） */
export const BTN_SM_BOLD =
  "min-h-[36px] rounded-full border border-line px-4 text-xs font-bold";

/** 危险次要按钮：红字粗体小字胶囊 */
export const BTN_SM_DANGER =
  "min-h-[36px] rounded-full border border-line px-4 text-xs" +
  " font-bold text-danger disabled:opacity-50";

/** 主按钮：36px 天蓝实底胶囊 */
export const BTN_SM_SKY =
  "min-h-[36px] rounded-full bg-sky px-4 text-xs font-bold" +
  " text-white disabled:opacity-50";

/** 主按钮：40px 天蓝大字胶囊 */
export const BTN_MD_SKY =
  "min-h-[40px] rounded-full bg-sky px-5 text-sm font-bold" +
  " text-white disabled:opacity-50";

/** 主按钮：44px 天蓝实底（整行提交位） */
export const BTN_LG_SKY =
  "min-h-[44px] rounded-full bg-sky font-bold text-white" +
  " active:scale-[0.97] disabled:opacity-50";

/** 迷你按钮：32px 胶囊描边 */
export const BTN_XS_GHOST =
  "min-h-[32px] rounded-full border border-line px-3 text-xs font-bold";

/** 主按钮：36px 主蓝**小圆角**（与 40px 输入框同排时的方角形态，非胶囊） */
export const BTN_SM_SQUARE =
  "min-h-[36px] rounded-[var(--r-sm)] bg-sky px-4 text-sm font-bold" +
  " text-white disabled:opacity-50";

/** 输入框：40px 圆角小方块 */
export const INPUT_MD =
  "min-h-[40px] rounded-[var(--r-sm)] border border-line px-2";

/** 输入框：40px 定宽（w-32） */
export const INPUT_W32 =
  "min-h-[40px] w-32 rounded-[var(--r-sm)] border border-line px-2";

/** 输入框：40px 占满（flex-1） */
export const INPUT_GROW =
  "min-h-[40px] flex-1 rounded-[var(--r-sm)] border border-line px-2";

/** 输入框：40px 卡片底色 + 聚焦描边 */
export const INPUT_CARD =
  "min-h-[40px] rounded-[var(--r-sm)] border border-line" +
  " bg-[var(--surface-card)] px-2";

/** 输入框：40px 云底 + sky 聚焦 */
export const INPUT_CLOUD =
  "min-h-[40px] rounded-[var(--r-sm)] border border-line bg-cloud" +
  " px-3 text-sm outline-none focus:border-sky";

/** 输入框：44px 卡片底 + 聚焦环 */
export const INPUT_LG =
  "min-h-[44px] rounded-[var(--r-sm)] border border-line" +
  " bg-[var(--surface-card)] px-3 outline-none focus:ring-2 focus:ring-sky/40";

/** 面板：大圆角卡片（r-lg） */
export const PANEL_LG =
  "rounded-[var(--r-lg)] border border-line" +
  " bg-[var(--surface-card)] p-4 shadow-[var(--shadow-card)]";

/** 面板：中圆角卡片（r-md） */
export const PANEL_MD =
  "rounded-[var(--r-md)] border border-line" +
  " bg-[var(--surface-card)] p-4 shadow-[var(--shadow-card)]";

/** 面板：中圆角卡片（无阴影） */
export const PANEL_MD_FLAT =
  "rounded-[var(--r-md)] border border-line"
  + " bg-[var(--surface-card)] p-4";

/** 面板：大圆角 + 纵向布局（表单/信息块） */
export const PANEL_LG_COL =
  "flex flex-col gap-3 rounded-[var(--r-lg)] border border-line" +
  " bg-[var(--surface-card)] p-4 shadow-[var(--shadow-card)]";

/** 面板：居中提示卡（空态/说明） */
export const PANEL_CENTER =
  "rounded-[var(--r-md)] border border-line bg-[var(--surface-card)]" +
  " p-6 text-center text-sm text-sub";

/** 表格单元格：卡片底次级文字 */
export const CELL_CARD_SUB =
  "border border-line bg-[var(--surface-card)] text-sub";

/** 旧版包子皮肤输入框（兼容） */
export const INPUT_BAOZI =
  "min-h-[32px] rounded-[var(--r-sm)] border border-[var(--baozi-line)]" +
  " bg-[var(--baozi-paper)] px-2 text-sm text-ink outline-none" +
  " focus:border-[var(--baozi-orange)]";

/** 旧版包子皮肤面板（兼容） */
export const PANEL_BAOZI =
  "rounded-[var(--r-md)] border border-[var(--baozi-line-soft)]" +
  " bg-[var(--baozi-paper)] p-4 shadow-[var(--shadow-card)]";
