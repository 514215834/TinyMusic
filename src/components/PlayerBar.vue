<script setup lang="ts">
import { computed, ref, watch, type Component } from "vue";
import { convertFileSrc } from "@tauri-apps/api/core";
import {
  ArrowRight,
  ListMusic,
  Maximize2,
  Pause,
  PictureInPicture2,
  Play,
  Repeat,
  Repeat1,
  Shuffle,
  SkipBack,
  SkipForward,
  Volume2,
  VolumeX,
} from "lucide-vue-next";
import QueuePanel from "./QueuePanel.vue";
import { libraryApi } from "../services/library";
import { overlayApi } from "../services/overlay";
import { usePlayerStore } from "../stores/player";
import { formatTime } from "../utils";
import { t } from "../i18n";

const player = usePlayerStore();
const coverUrl = ref<string | null>(null);

watch(
  () => player.currentTrack?.coverFile,
  async (file) => {
    if (!file) {
      coverUrl.value = null;
      return;
    }
    const path = await libraryApi.coverPath(file);
    coverUrl.value = path ? convertFileSrc(path) : null;
  },
  { immediate: true },
);

const MODE_META: Record<string, { icon: Component; label: string }> = {
  sequential: { icon: ArrowRight, label: "player.mode.sequential" },
  "repeat-all": { icon: Repeat, label: "player.mode.repeatAll" },
  "repeat-one": { icon: Repeat1, label: "player.mode.repeatOne" },
  shuffle: { icon: Shuffle, label: "player.mode.shuffle" },
};
const modeMeta = computed(() => MODE_META[player.mode] ?? MODE_META["sequential"]);

const dragValue = ref<number | null>(null);
function onSeekInput(e: Event) {
  dragValue.value = Number((e.target as HTMLInputElement).value);
}
function onSeekChange(e: Event) {
  player.seek(Number((e.target as HTMLInputElement).value));
  dragValue.value = null;
}
const posShown = computed(() => dragValue.value ?? player.positionSec);

/* 播放队列面板 */
const queueOpen = ref(false);
function toggleQueue() {
  queueOpen.value = !queueOpen.value;
}

/** 迷你悬浮播放器开/关（M2 §6） */
function toggleOverlay() {
  void overlayApi.toggle().catch(() => {});
}
</script>

<template>
  <div class="player-bar">
    <div class="now">
      <img v-if="coverUrl" :src="coverUrl" class="cover" alt="封面" />
      <div v-else class="cover cover-empty"></div>
      <div class="meta">
        <div class="title" :title="player.currentTrack?.title">
          {{ player.currentTrack?.title ?? t("player.notPlaying") }}
        </div>
        <div class="artist dim">{{ player.currentTrack?.artist ?? "—" }}</div>
      </div>
    </div>

    <div class="controls">
      <button class="icon" :title="t('player.prev')" @click="player.prev()">
        <SkipBack :size="17" />
      </button>
      <button
        class="icon play"
        :title="player.isPlaying ? t('player.pause') : t('player.play')"
        @click="player.toggle()"
      >
        <Pause v-if="player.isPlaying" :size="19" />
        <Play v-else :size="19" fill="currentColor" />
      </button>
      <button class="icon" :title="t('player.next')" @click="player.next()">
        <SkipForward :size="17" />
      </button>
    </div>

    <div class="progress">
      <span class="time dim">{{ formatTime(posShown) }}</span>
      <input
        class="slider"
        type="range"
        min="0"
        :max="player.durationSec || 0"
        step="0.1"
        :value="posShown"
        @input="onSeekInput"
        @change="onSeekChange"
      />
      <span class="time dim">{{ formatTime(player.durationSec) }}</span>
    </div>

    <div class="right">
      <button class="icon" :title="t(modeMeta.label)" @click="player.cycleMode()">
        <component :is="modeMeta.icon" :size="16" />
      </button>
      <button
        class="icon"
        :class="{ on: queueOpen }"
        :title="`${t('player.queue')}（${player.userQueue.length}）`"
        @click="toggleQueue"
      >
        <ListMusic :size="16" />
      </button>
      <button
        class="icon"
        :title="player.muted ? t('player.unmute') : t('player.mute')"
        @click="player.toggleMute()"
      >
        <VolumeX v-if="player.muted" :size="16" />
        <Volume2 v-else :size="16" />
      </button>
      <input
        class="slider volume"
        type="range"
        min="0"
        max="1"
        step="0.01"
        :value="player.muted ? 0 : player.volume"
        @input="player.setVolume(Number(($event.target as HTMLInputElement).value))"
      />
      <button
        class="icon"
        :class="{ on: player.nowPlayingOpen }"
        :title="t('player.nowPlaying')"
        @click="player.toggleNowPlaying()"
      >
        <Maximize2 :size="14" />
      </button>
      <button class="icon" :title="t('settings.miniPlayer')" @click="toggleOverlay()">
        <PictureInPicture2 :size="15" />
      </button>
    </div>

    <QueuePanel v-if="queueOpen" />
  </div>
</template>

<style scoped>
.player-bar {
  height: 100%;
  display: grid;
  grid-template-columns: minmax(180px, 1fr) auto minmax(220px, 2fr) minmax(200px, 1fr);
  gap: 16px;
  align-items: center;
  padding: 0 16px;
}

.now {
  display: flex;
  align-items: center;
  gap: 10px;
  min-width: 0;
}

.cover {
  width: 48px;
  height: 48px;
  border-radius: 8px;
  object-fit: cover;
  flex-shrink: 0;
}

.cover-empty {
  background: var(--bg-hover);
}

.meta {
  min-width: 0;
}

.title {
  font-weight: 600;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.artist {
  font-size: 12px;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.controls {
  display: flex;
  align-items: center;
  gap: 4px;
}

.icon {
  width: 36px;
  height: 36px;
  border: none;
  background: transparent;
  border-radius: 50%;
  padding: 0;
  cursor: pointer;
  display: inline-flex;
  align-items: center;
  justify-content: center;
  flex-shrink: 0;
}

/* SVG 按 block 渲染，消除内联基线偏移导致的视觉不居中 */
.icon svg {
  display: block;
}

.icon:hover:not(:disabled) {
  background: var(--bg-hover);
}

.icon.on {
  color: var(--accent);
}

.icon.play {
  width: 42px;
  height: 42px;
  background: var(--accent);
  color: #fff;
}

.icon.play:hover:not(:disabled) {
  filter: brightness(1.08);
  background: var(--accent);
}

.icon:disabled {
  opacity: 0.4;
  cursor: default;
}

.progress {
  display: flex;
  align-items: center;
  gap: 10px;
  min-width: 0;
}

.time {
  font-size: 12px;
  min-width: 40px;
  text-align: center;
}

.right {
  display: flex;
  align-items: center;
  gap: 4px;
  justify-content: flex-end;
}

.slider {
  flex: 1;
  accent-color: var(--accent);
  min-width: 80px;
}

.slider.volume {
  flex: 0 1 100px;
}
</style>
