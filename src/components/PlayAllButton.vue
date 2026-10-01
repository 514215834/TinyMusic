<script setup lang="ts">
import { ref } from "vue";
import { Play } from "lucide-vue-next";
import type { Track } from "../services/library";
import { usePlayerStore } from "../stores/player";
import { t } from "../i18n";

/**
 * 一键播放整个列表：传入现成数组（专辑/歌单/文件夹等），
 * 或传 load()（曲库视图按需拉全量）。
 */
const props = defineProps<{ tracks?: Track[]; load?: () => Promise<Track[]> }>();
const player = usePlayerStore();
const busy = ref(false);

async function onClick() {
  if (busy.value) return;
  busy.value = true;
  try {
    const list = props.tracks ?? (props.load ? await props.load() : []);
    if (list.length) player.play(list, 0);
  } finally {
    busy.value = false;
  }
}
</script>

<template>
  <button class="play-all" :disabled="busy" @click="onClick">
    <Play :size="13" fill="currentColor" />
    {{ t("player.playAll") }}
  </button>
</template>

<style scoped>
.play-all {
  display: inline-flex;
  align-items: center;
  gap: 6px;
  padding: 6px 14px;
  border: 1px solid var(--accent);
  border-radius: 999px;
  background: color-mix(in srgb, var(--accent) 14%, transparent);
  color: var(--accent);
  font-size: 13px;
  font-weight: 600;
  cursor: pointer;
  transition:
    background-color 0.15s ease,
    transform 0.12s ease;
}

.play-all svg {
  display: block;
}

.play-all:hover:not(:disabled) {
  background: color-mix(in srgb, var(--accent) 24%, transparent);
}

.play-all:active:not(:disabled) {
  transform: scale(0.96);
}

.play-all:disabled {
  opacity: 0.5;
  cursor: default;
}
</style>
