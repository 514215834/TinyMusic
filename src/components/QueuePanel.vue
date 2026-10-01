<script setup lang="ts">
import { ref } from "vue";
import { ListMusic, X } from "lucide-vue-next";
import { usePlayerStore } from "../stores/player";
import { useCovers } from "../composables/useCovers";
import { formatTime } from "../utils";
import { t } from "../i18n";

const player = usePlayerStore();
const covers = useCovers(() => [...player.userQueue]);

/* HTML5 拖拽重排：记录拖起点，落到目标行完成移位 */
const dragIndex = ref<number | null>(null);
const overIndex = ref<number | null>(null);

function onDragStart(index: number) {
  dragIndex.value = index;
}
function onDragOver(index: number) {
  overIndex.value = index;
}
function onDrop() {
  if (dragIndex.value != null && overIndex.value != null) {
    player.reorderUserQueue(dragIndex.value, overIndex.value);
  }
  dragIndex.value = null;
  overIndex.value = null;
}
</script>

<template>
  <div class="queue-panel">
    <header class="q-head">
      <ListMusic :size="14" />
      {{ t("player.queue") }}
      <span class="dim">{{ player.userQueue.length }} 首</span>
      <div class="spacer"></div>
      <button v-if="player.userQueue.length" class="clear dim" @click="player.clearUserQueue()">
        {{ t("player.queueClear") }}
      </button>
    </header>
    <div v-if="!player.userQueue.length" class="q-empty dim">
      {{ t("player.queueEmpty") }}
    </div>
    <div v-else class="q-list">
      <div
        v-for="(tr, i) in player.userQueue"
        :key="`${tr.id}-${i}`"
        class="q-item"
        :class="{ dragging: dragIndex === i, over: overIndex === i && dragIndex !== i }"
        draggable="true"
        @dragstart="onDragStart(i)"
        @dragover.prevent="onDragOver(i)"
        @dragleave="overIndex = null"
        @dragend="onDrop"
        @drop.prevent="onDrop"
        @dblclick="player.removeFromQueue(i)"
      >
        <span class="q-cover">
          <img v-if="covers[tr.id]" :src="covers[tr.id] ?? undefined" alt="" />
          <ListMusic v-else :size="13" />
        </span>
        <span class="q-meta">
          <span class="q-name" :title="tr.title">{{ tr.title }}</span>
          <span class="q-artist" :title="tr.artist ?? ''">{{ tr.artist ?? "—" }}</span>
        </span>
        <span class="q-dur">{{ tr.durationSec ? formatTime(tr.durationSec) : "—" }}</span>
        <button class="q-x" :title="t('player.removeFromQueue')" @click="player.removeFromQueue(i)">
          <X :size="12" />
        </button>
      </div>
    </div>
  </div>
</template>

<style scoped>
.queue-panel {
  position: absolute;
  right: 12px;
  bottom: 76px;
  width: 340px;
  max-width: calc(100vw - 24px);
  max-height: 420px;
  display: flex;
  flex-direction: column;
  background: color-mix(in srgb, var(--bg-elev) 72%, transparent);
  backdrop-filter: blur(24px) saturate(1.4);
  -webkit-backdrop-filter: blur(24px) saturate(1.4);
  border: 1px solid var(--border);
  border-radius: 12px;
  box-shadow: 0 8px 28px rgba(0, 0, 0, 0.18);
  z-index: 50;
  animation: fade-up 0.18s ease both;
}

.q-head {
  display: flex;
  align-items: center;
  gap: 6px;
  padding: 10px 12px;
  font-weight: 600;
  border-bottom: 1px solid var(--border);
}

.spacer {
  flex: 1;
}

.clear {
  border: none;
  background: transparent;
  cursor: pointer;
  font-size: 12px;
  padding: 2px 6px;
  border-radius: 6px;
}

.clear:hover {
  background: var(--bg-hover);
  color: var(--text);
}

.q-empty {
  padding: 24px 16px;
  font-size: 12px;
  text-align: center;
  line-height: 1.6;
}

.q-list {
  overflow: auto;
  padding: 4px;
}

.q-item {
  display: grid;
  grid-template-columns: auto minmax(0, 1fr) auto auto;
  align-items: center;
  gap: 10px;
  padding: 5px 8px;
  border-radius: 8px;
  cursor: grab;
  transition: background-color 0.12s ease;
}

.q-item:hover {
  background: var(--bg-hover);
}

.q-item.dragging {
  opacity: 0.4;
}

.q-item.over {
  box-shadow: inset 0 2px 0 var(--accent);
}

.q-cover {
  display: flex;
  align-items: center;
  justify-content: center;
  width: 32px;
  height: 32px;
  border-radius: 4px;
  background: var(--bg-hover);
  color: var(--text-dim);
  overflow: hidden;
  flex: none;
}

.q-cover img {
  width: 100%;
  height: 100%;
  object-fit: cover;
  display: block;
}

.q-meta {
  display: flex;
  flex-direction: column;
  gap: 1px;
  min-width: 0;
}

.q-name,
.q-artist {
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.q-name {
  font-size: 13px;
}

.q-artist {
  font-size: 11px;
}

.q-dur {
  font-size: 12px;
  color: var(--text-dim);
  font-variant-numeric: tabular-nums;
}

.q-x {
  display: inline-flex;
  align-items: center;
  justify-content: center;
  width: 20px;
  height: 20px;
  padding: 0;
  border: none;
  border-radius: 50%;
  background: transparent;
  color: var(--text-dim);
  cursor: pointer;
  opacity: 0;
  transition:
    opacity 0.15s ease,
    background-color 0.12s ease;
}

.q-item:hover .q-x {
  opacity: 1;
}

.q-x:hover {
  background: var(--border);
  color: var(--text);
}
</style>
