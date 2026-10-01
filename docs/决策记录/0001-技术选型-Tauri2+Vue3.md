# ADR-0001 技术选型：Tauri 2 + Vue 3

- 状态：已接受
- 日期：2026-09-27
- 决策人：用户评审确认（评审结论 D1）

## 背景

TinyMusic 是极简本地桌面音乐播放器，Windows 优先，单人开发，后续编码按文档规范执行。需要选择桌面应用技术栈，核心考量：体积与内存（"Tiny"定位）、音频格式覆盖、元数据解析生态、系统集成（托盘/媒体键/悬浮窗）、单人迭代效率。

## 决策

采用 **Tauri 2 + Vue 3 + TypeScript（前端）+ Rust（后端核心）**：

- 前端：Vue 3（`<script setup>`）+ Pinia + Vue Router + Vite
- 后端：Tauri 2 + lofty（元数据）+ rusqlite（存储）+ walkdir/notify（扫描监听）
- 音频：M0–M2 使用 renderer `<audio>`；M3 EQ 用 Web Audio、输出设备用 `setSinkId`

## 理由

| 维度 | Tauri 2 | Electron（备选） |
|---|---|---|
| 安装包 | ~10MB | ~85MB |
| 空闲内存 | ~80–120MB | ~200MB |
| 音频解码 | WebView2（Chromium 内核） | Chromium |
| 元数据生态 | lofty | music-metadata |
| 系统集成 | 官方插件齐全；SMTC 需自研 | MediaSession 原生齐备 |

Tauri 与"Tiny"定位契合，且已有本地播放器先例（Musicat、Audion）验证可行。

## 影响

- 正向：体积/内存小一个数量级；Windows 10 1809+ 自带 WebView2；安全边界清晰
- 代价：需 Rust 基础；M2 的 SMTC 需 windows-rs 自研封装（约百余行，已列入风险清单并要求独立模块化以便替换）；gapless 若 `<audio>` 方案不达标需在 M4 评估 rodio

## 备选方案

1. **Electron + Vue 3**：生态最成熟、SMTC 零成本，但体积/内存不满足 Tiny 定位 —— 落选
2. **Qt/C++（Strawberry 类）**：性能最佳但单人迭代效率低、UI 开发成本高 —— 落选

## 关联评审结论（2026-09-27）

1. D1 技术栈：Tauri 2 + Vue 3 ✅
2. D2 M0 范围：仅"扫描→列表→播放" ✅
3. D3 迭代顺序 M0→M1→M2→M3 ✅
4. D4 格式范围：MP3/M4A/FLAC/WAV/OGG，APE/WMA 不承诺 ✅
5. D5 文档中文命名：`docs/功能设计迭代文档.md`、`docs/技术设计文档.md`、`docs/决策记录/` ✅
6. 新增 P0 需求：迷你悬浮播放器（置顶/透明/可拖动/鼠标穿透），纳入 M2 ✅
