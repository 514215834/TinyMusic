<script setup lang="ts">
import { onMounted, ref } from "vue";
import { History } from "lucide-vue-next";
import TrackTable from "../components/TrackTable.vue";
import PlayAllButton from "../components/PlayAllButton.vue";
import { statsApi, type Track } from "../services/library";
import { t } from "../i18n";

type Tab = "recent" | "top";
const tab = ref<Tab>("recent");
const tracks = ref<Track[]>([]);
const loading = ref(false);

async function load() {
  loading.value = true;
  try {
    tracks.value = tab.value === "recent" ? await statsApi.recent(200) : await statsApi.top(200);
  } finally {
    loading.value = false;
  }
}

function switchTab(next: Tab) {
  if (tab.value === next) return;
  tab.value = next;
  void load();
}

onMounted(load);
</script>

<template>
  <section class="history">
    <header class="toolbar">
      <History :size="18" />
      <h2>{{ t("history.title") }}</h2>
      <div class="segment">
        <button :class="{ active: tab === 'recent' }" @click="switchTab('recent')">
          {{ t("history.recent") }}
        </button>
        <button :class="{ active: tab === 'top' }" @click="switchTab('top')">
          {{ t("history.top") }}
        </button>
      </div>
      <span class="dim">{{ t("albums.trackCount", { n: tracks.length }) }}</span>
      <div class="spacer"></div>
      <PlayAllButton :tracks="tracks" />
    </header>
    <TrackTable :tracks="tracks" :empty-text="t('history.empty')" />
    <div v-if="loading" class="dim loading">{{ t("library.loading") }}</div>
  </section>
</template>

<style scoped>
.history {
  height: 100%;
  display: flex;
  flex-direction: column;
  gap: 12px;
}

.toolbar {
  display: flex;
  align-items: center;
  gap: 10px;
}

.spacer {
  flex: 1;
}

.toolbar h2 {
  margin: 0;
  font-size: 18px;
}

.segment {
  display: inline-flex;
  border: 1px solid var(--border);
  border-radius: 8px;
  overflow: hidden;
}

.segment button {
  padding: 5px 12px;
  border: none;
  background: var(--bg-elev);
  color: var(--text);
  cursor: pointer;
  font-size: 13px;
}

.segment button + button {
  border-left: 1px solid var(--border);
}

.segment button.active {
  background: var(--accent);
  color: #fff;
}

.loading {
  padding: 8px;
  text-align: center;
  font-size: 12px;
}
</style>
