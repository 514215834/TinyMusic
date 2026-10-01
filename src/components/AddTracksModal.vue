<script setup lang="ts">
import { onMounted, ref, watch } from "vue";
import { Check, Plus, Search, X } from "lucide-vue-next";
import { libraryApi, type Track } from "../services/library";
import { formatTime } from "../utils";
import { t } from "../i18n";

const props = defineProps<{ playlistId: number; existingIds: number[] }>();
const emit = defineEmits<{ close: []; add: [track: Track] }>();

const q = ref("");
const all = ref<Track[]>([]);
const searchBox = ref<HTMLInputElement | null>(null);

onMounted(async () => {
  // 取前 500 首作为候选池；超过部分靠搜索检索
  const page = await libraryApi.tracksQuery(1, 500);
  all.value = page.items;
  searchBox.value?.focus();
});

const results = ref<Track[] | null>(null);
let timer: ReturnType<typeof setTimeout> | undefined;
watch(q, (v) => {
  clearTimeout(timer);
  timer = setTimeout(async () => {
    const query = v.trim();
    if (!query) {
      results.value = null;
      return;
    }
    const page = await libraryApi.searchTracks(query, 1, 100);
    results.value = page.items;
  }, 150);
});

const shown = () => results.value ?? all.value;
const exists = (t: Track) => props.existingIds.includes(t.id);
</script>

<template>
  <div class="modal-mask" @click.self="emit('close')">
    <div class="modal">
      <header class="m-head">
        <Search :size="14" />
        <input
          ref="searchBox"
          v-model="q"
          class="m-search"
          type="text"
          :placeholder="t('common.search')"
        />
        <button class="m-close" :title="t('common.close')" @click="emit('close')">
          <X :size="14" />
        </button>
      </header>
      <div class="m-list">
        <div v-for="tr in shown()" :key="tr.id" class="m-row">
          <span class="m-title" :title="tr.title">{{ tr.title }}</span>
          <span class="m-artist dim">{{ tr.artist ?? "—" }}</span>
          <span class="m-dur dim">{{ tr.durationSec ? formatTime(tr.durationSec) : "—" }}</span>
          <button
            class="m-add"
            :disabled="exists(tr)"
            :title="exists(tr) ? t('player.queue') : t('playlist.addTracks')"
            @click="!exists(tr) && emit('add', tr)"
          >
            <Check v-if="exists(tr)" :size="14" />
            <Plus v-else :size="14" />
          </button>
        </div>
        <div v-if="!shown().length" class="m-empty dim">{{ t("library.noMatch") }}</div>
      </div>
    </div>
  </div>
</template>

<style scoped>
.modal-mask {
  position: fixed;
  inset: 0;
  background: rgba(0, 0, 0, 0.4);
  backdrop-filter: blur(10px);
  -webkit-backdrop-filter: blur(10px);
  display: flex;
  align-items: center;
  justify-content: center;
  z-index: 100;
  animation: fade-in 0.15s ease both;
}

.modal {
  animation: pop-in 0.2s cubic-bezier(0.2, 0.9, 0.3, 1.15) both;
  width: 560px;
  max-width: calc(100vw - 48px);
  height: 480px;
  max-height: calc(100vh - 48px);
  display: flex;
  flex-direction: column;
  background: var(--bg-elev);
  border: 1px solid var(--border);
  border-radius: 12px;
  overflow: hidden;
}

.m-head {
  display: flex;
  align-items: center;
  gap: 8px;
  padding: 12px 14px;
  border-bottom: 1px solid var(--border);
  color: var(--text-dim);
}

.m-search {
  flex: 1;
  border: none;
  outline: none;
  background: transparent;
  color: var(--text);
  font-size: 14px;
}

.m-close {
  display: inline-flex;
  align-items: center;
  justify-content: center;
  width: 26px;
  height: 26px;
  padding: 0;
  border: none;
  border-radius: 6px;
  background: transparent;
  color: var(--text-dim);
  cursor: pointer;
}

.m-close:hover {
  background: var(--bg-hover);
  color: var(--text);
}

.m-list {
  flex: 1;
  overflow: auto;
  padding: 6px;
}

.m-row {
  display: grid;
  grid-template-columns: 1fr 150px 56px auto;
  gap: 10px;
  align-items: center;
  padding: 7px 10px;
  border-radius: 8px;
}

.m-row:hover {
  background: var(--bg-hover);
}

.m-title,
.m-artist {
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
  font-size: 13px;
}

.m-dur {
  font-size: 12px;
  text-align: right;
}

.m-add {
  display: inline-flex;
  align-items: center;
  justify-content: center;
  width: 26px;
  height: 26px;
  padding: 0;
  border: 1px solid var(--border);
  border-radius: 8px;
  background: transparent;
  color: var(--text);
  cursor: pointer;
}

.m-add:disabled {
  color: var(--accent);
  border-color: transparent;
  cursor: default;
}

.m-add svg {
  display: block;
}

.m-empty {
  padding: 32px;
  text-align: center;
}
</style>
