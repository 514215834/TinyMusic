/**
 * 等功率（equal-power）交叉渐变曲线（M8 crossfade）：
 * 两路增益分别走 cos/sin 四分之一周期，能量和恒为 1，
 * 中点不出现线性渐变的 -3dB 响度凹陷。
 * @param progress 0（完全旧曲）→ 1（完全新曲）
 */
export function crossfadeGains(progress: number): { out: number; enter: number } {
  const p = Math.min(1, Math.max(0, progress));
  const rad = (p * Math.PI) / 2;
  return { out: Math.cos(rad), enter: Math.sin(rad) };
}
