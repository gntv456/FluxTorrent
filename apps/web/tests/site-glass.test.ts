import { describe, expect, it } from "vitest";
import { glassEnabled, glassStyle, glassValues } from "@/lib/site-glass";

/** 板块磨砂玻璃（0448）：值会直接进 CSS calc()，越界与非数必须夹紧 */
describe("site-glass", () => {
  it("缺省与非法值回落默认档（88% / 12px）", () => {
    expect(glassValues(null)).toEqual({ alpha: 88, blur: 12 });
    expect(glassValues({})).toEqual({ alpha: 88, blur: 12 });
    expect(glassValues({ alpha: NaN, blur: undefined })).toEqual({
      alpha: 88,
      blur: 12,
    });
  });

  it("越界夹紧：alpha 0~100、blur 0~30", () => {
    expect(glassValues({ alpha: 250, blur: 999 })).toEqual({
      alpha: 100,
      blur: 30,
    });
    expect(glassValues({ alpha: -20, blur: -1 })).toEqual({
      alpha: 0,
      blur: 0,
    });
  });

  it("alpha=100 视为关闭；0 仍是启用（全透明但挂磨砂）", () => {
    expect(glassEnabled({ alpha: 100 })).toBe(false);
    expect(glassStyle({ alpha: 100, blur: 20 })).toBeNull();
    expect(glassEnabled({ alpha: 0 })).toBe(true);
  });

  it("启用时写成 --glass-alpha / --glass-blur 两个变量", () => {
    expect(glassStyle({ alpha: 70, blur: 18 })).toEqual({
      "--glass-alpha": "0.7",
      "--glass-blur": "18px",
    });
  });
});
