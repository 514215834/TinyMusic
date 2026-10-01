<script setup lang="ts">
import { computed, onBeforeUnmount, onMounted, ref } from "vue";
import { emit, listen, type UnlistenFn } from "@tauri-apps/api/event";
import { convertFileSrc } from "@tauri-apps/api/core";
import { Music2, Pause, Play, SkipBack, SkipForward, X } from "lucide-vue-next";
import { libraryApi } from "../services/library";
import { overlayApi } from "../services/overlay";
import { t } from "../i18n";

// 迷你悬浮窗是纯薄镜像（技术设计文档 §6.4）：不持有播放状态，
// 渲染 playback:changed，控制操作经 overlay:command 回流主窗口。
//
// 显隐模型（§6.2/§6.3）：
// - 未悬停：整卡不透明，仅显示封面；
// - 悬停：封面层变透明（settings 透明度），浮现渐变/信息/控制按钮；
// - 调整态（非锁定）悬停由 CSS :hover 即时驱动，且按钮可操作、整卡可拖动；
// - 锁定态窗口鼠标穿透收不到 hover，由 Rust watcher 轮询推送 hover 字段驱动
//   同样的视觉（内容压暗示意不可操作），操作仍不可用。

interface PlaybackSnapshot {
  id: number;
  title: string;
  artist: string | null;
  coverFile: string | null;
  isPlaying: boolean;
  mode: string;
  positionSec: number;
  durationSec: number;
}

const track = ref<PlaybackSnapshot | null>(null);
const isPlaying = ref(false);
const positionSec = ref(0);
const durationSec = ref(0);
const locked = ref(true);
const hovered = ref(false);
const opacity = ref(0.6);
const coverUrl = ref<string | null>(null);

// 广播随 timeupdate 高频到达：封面键未变化时不重复走 IPC 解析
let coverKey: string | null = null;

const unlisteners: UnlistenFn[] = [];

function applySnapshot(snapshot: PlaybackSnapshot | null) {
  track.value = snapshot;
  isPlaying.value = snapshot?.isPlaying ?? false;
  positionSec.value = snapshot?.positionSec ?? 0;
  durationSec.value = snapshot?.durationSec ?? 0;
  const key = snapshot?.coverFile ?? null;
  if (key === coverKey) return;
  coverKey = key;
  if (key) {
    void libraryApi
      .coverPath(key)
      .then((path) => {
        coverUrl.value = path ? convertFileSrc(path) : null;
      })
      .catch(() => {
        coverUrl.value = null;
      });
  } else {
    coverUrl.value = null;
  }
}

const progress = computed(() =>
  durationSec.value > 0 ? Math.min(100, (positionSec.value / durationSec.value) * 100) : 0,
);

function send(action: string) {
  void emit("overlay:command", action).catch(() => {});
}

function close() {
  void overlayApi.hide().catch(() => {});
}

onMounted(async () => {
  unlisteners.push(
    await listen<PlaybackSnapshot | null>("playback:changed", (e) => applySnapshot(e.payload)),
    await listen<{ locked: boolean; hover: boolean; opacity: number }>("overlay:state", (e) => {
      locked.value = e.payload.locked;
      hovered.value = e.payload.hover;
      opacity.value = e.payload.opacity;
    }),
  );
  // 暂停中主窗口不会广播：打开时主动请求一次当前快照
  void emit("playback:sync-request").catch(() => {});
});

onBeforeUnmount(() => unlisteners.forEach((fn) => fn()));
</script>

<template>
  <section class="mini" :class="{ locked, hovered }" :style="{ '--mini-alpha': opacity }">
    <div class="visual" data-tauri-drag-region>
      <img
        v-if="coverUrl"
        :src="coverUrl"
        class="art"
        alt=""
        draggable="false"
        data-tauri-drag-region
      />
      <div v-else class="art art-empty" data-tauri-drag-region>
        <Music2 :size="30" />
      </div>
    </div>
    <div class="scrim" data-tauri-drag-region></div>

    <div class="content">
      <button class="close" :title="t('overlay.close')" @click="close()">
        <X :size="13" />
      </button>

      <footer class="panel" data-tauri-drag-region>
        <div class="meta" data-tauri-drag-region>
          <div class="title" :title="track?.title" data-tauri-drag-region>
            {{ track?.title ?? t("player.notPlaying") }}
          </div>
          <div class="artist" data-tauri-drag-region>{{ track?.artist ?? "—" }}</div>
        </div>
        <div class="controls">
          <button class="ctrl" :title="t('player.prev')" @click="send('prev')">
            <SkipBack :size="15" />
          </button>
          <button
            class="ctrl play"
            :title="isPlaying ? t('player.pause') : t('player.play')"
            @click="send('toggle')"
          >
            <Pause v-if="isPlaying" :size="17" />
            <Play v-else :size="17" fill="currentColor" />
          </button>
          <button class="ctrl" :title="t('player.next')" @click="send('next')">
            <SkipForward :size="15" />
          </button>
        </div>
      </footer>

      <div class="progress">
        <div class="fill" :style="{ width: `${progress}%` }"></div>
      </div>
    </div>
  </section>
</template>

<style>
/* 悬浮窗整体透明：body 背景必须透出桌面（非 scoped 覆盖 main.css） */
body:has(.mini) {
  background: transparent !important;
}
</style>

<style scoped>
.mini {
  position: relative;
  height: 100vh;
  color: #fff;
  border-radius: 12px;
  overflow: hidden;
  /* 不用 CSS 外投影：投影会画进窗口矩形的圆角缺口，浅色桌面露灰；
     深度感交给暗色发丝描边 + Win11 DWM 原生圆角合成（overlay.rs）。
     描边用暗色：白色描边会在浅色桌面四角与封面圆角处加重白边 */
  border: 1px solid rgba(0, 0, 0, 0.22);
  user-select: none;
}

.locked {
  /* 穿透由 Rust set_ignore_cursor_events 保证；CSS 仅作视觉提示 */
  pointer-events: none;
}

/* 封面层：未悬停不透明，悬停时按 settings 透明度变透明 */
.visual {
  position: absolute;
  inset: 0;
  opacity: 1;
  transition: opacity 0.22s ease;
  cursor: grab;
}

.mini:hover .visual,
.mini.hovered .visual {
  opacity: var(--mini-alpha);
}

.art {
  position: absolute;
  inset: 0;
  width: 100%;
  height: 100%;
  object-fit: cover;
}

.art-empty {
  display: flex;
  align-items: center;
  justify-content: center;
  background: linear-gradient(135deg, #33353d 0%, #1b1c22 100%);
  color: rgba(255, 255, 255, 0.32);
}

/* 底部渐变：仅悬停时浮现，保证文字可读 */
.scrim {
  position: absolute;
  left: 0;
  right: 0;
  bottom: 0;
  height: 88px;
  background: linear-gradient(180deg, rgba(0, 0, 0, 0) 0%, rgba(0, 0, 0, 0.64) 100%);
  opacity: 0;
  transition: opacity 0.2s ease;
  cursor: grab;
}

.mini:hover .scrim,
.mini.hovered .scrim {
  opacity: 1;
}

/* 内容层（信息/控制/进度）：仅悬停浮现；默认不拦截命中，方便点到下方拖动区 */
.content {
  position: absolute;
  inset: 0;
  opacity: 0;
  transition: opacity 0.2s ease;
  pointer-events: none;
}

.mini:hover .content,
.mini.hovered .content {
  opacity: 1;
}

/* 锁定态悬停：内容压暗示意"仅展示、不可操作" */
.mini.hovered.locked .content {
  opacity: 0.75;
}

.close {
  position: absolute;
  top: 8px;
  right: 8px;
  width: 24px;
  height: 24px;
  display: inline-flex;
  align-items: center;
  justify-content: center;
  border: none;
  border-radius: 50%;
  background: rgba(20, 20, 20, 0.55);
  color: rgba(255, 255, 255, 0.9);
  cursor: pointer;
  pointer-events: auto;
}

.close:hover {
  background: rgba(20, 20, 20, 0.8);
}

.panel {
  position: absolute;
  left: 12px;
  right: 10px;
  bottom: 14px;
  display: flex;
  align-items: center;
  gap: 10px;
  cursor: grab;
  pointer-events: auto;
}

.meta {
  flex: 1;
  min-width: 0;
}

.title {
  font-size: 13px;
  font-weight: 600;
  line-height: 1.3;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
  text-shadow: 0 1px 3px rgba(0, 0, 0, 0.45);
}

.artist {
  margin-top: 2px;
  font-size: 11px;
  line-height: 1.3;
  color: rgba(255, 255, 255, 0.72);
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
  text-shadow: 0 1px 2px rgba(0, 0, 0, 0.45);
}

.controls {
  display: flex;
  align-items: center;
  gap: 2px;
  flex-shrink: 0;
}

.ctrl {
  width: 32px;
  height: 32px;
  display: inline-flex;
  align-items: center;
  justify-content: center;
  border: none;
  border-radius: 50%;
  background: transparent;
  color: rgba(255, 255, 255, 0.92);
  cursor: pointer;
  flex-shrink: 0;
}

/* SVG 按 block 渲染，消除内联基线偏移导致的视觉不居中 */
.ctrl svg {
  display: block;
}

.ctrl:hover {
  background: rgba(255, 255, 255, 0.16);
}

.ctrl.play {
  width: 36px;
  height: 36px;
}

/* Win11 式底缘细进度条 */
.progress {
  position: absolute;
  left: 0;
  right: 0;
  bottom: 0;
  height: 3px;
  background: rgba(255, 255, 255, 0.22);
}

.fill {
  height: 100%;
  background: rgba(255, 255, 255, 0.95);
  transition: width 0.25s linear;
}

/* 锁定态整卡穿透：内容层禁用一切命中（真正的拦截在 Rust ignore_cursor_events） */
.locked .content,
.locked .content * {
  pointer-events: none !important;
}
</style>
