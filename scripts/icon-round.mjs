// 应用图标圆角化（配合 `tauri icon` 全套生成）。依赖 sharp（不写入 package.json）：
//   npm install --no-save sharp
//   node scripts/icon-round.mjs <源图> <输出主图>
// 源图裁到 1024×1024 后按 22.5%（macOS 风格比例）圆角矩形 alpha 遮罩，
// 输出带透明圆角的 PNG 主图；再执行 `npx tauri icon <主图> -o src-tauri/icons`
// 重新生成全套（生成后需移除 android/ios 目录，本项目仅 Windows 桌面）。
import sharp from "sharp";

const [input, output] = process.argv.slice(2);
if (!input || !output) {
  console.error("用法: node scripts/icon-round.mjs <源图> <输出主图>");
  process.exit(1);
}

const SIZE = 1024;
const RADIUS = Math.round(SIZE * 0.225);

const mask = Buffer.from(
  `<svg width="${SIZE}" height="${SIZE}"><rect width="${SIZE}" height="${SIZE}" rx="${RADIUS}" ry="${RADIUS}" fill="#fff"/></svg>`,
);

await sharp(input)
  .resize(SIZE, SIZE)
  .composite([{ input: mask, blend: "dest-in" }])
  .png()
  .toFile(output);

const meta = await sharp(output).metadata();
console.log(`已生成 ${output} (${meta.width}x${meta.height}, 圆角 ${RADIUS}px)`);
