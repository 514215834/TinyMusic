import { describe, expect, it } from "vitest";
import { crossfadeGains } from "./crossfade";

describe("crossfadeGains 等功率曲线", () => {
  it("端点：起点全旧曲、终点全新曲", () => {
    expect(crossfadeGains(0)).toEqual({ out: 1, enter: 0 });
    expect(crossfadeGains(1).out).toBeCloseTo(0, 12);
    expect(crossfadeGains(1).enter).toBeCloseTo(1, 12);
  });

  it("中点两路增益相等（约 0.707），能量和恒为 1", () => {
    const mid = crossfadeGains(0.5);
    expect(mid.out).toBeCloseTo(Math.SQRT1_2, 12);
    expect(mid.enter).toBeCloseTo(Math.SQRT1_2, 12);
    for (const p of [0, 0.25, 0.5, 0.75, 1]) {
      const { out, enter } = crossfadeGains(p);
      expect(out * out + enter * enter).toBeCloseTo(1, 12);
    }
  });

  it("越界进度钳制到 [0,1]，单调不回退", () => {
    expect(crossfadeGains(-3).out).toBe(1);
    expect(crossfadeGains(9).enter).toBe(1);
    let prevOut = 2;
    for (let p = 0; p <= 1.0001; p += 0.05) {
      const { out } = crossfadeGains(p);
      expect(out).toBeLessThanOrEqual(prevOut);
      prevOut = out;
    }
  });
});
