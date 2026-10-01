<script setup lang="ts">
import { computed, onMounted, ref, watch } from "vue";
import { useRoute, useRouter } from "vue-router";
import { Folder } from "lucide-vue-next";
import TrackTable from "../components/TrackTable.vue";
import PlayAllButton from "../components/PlayAllButton.vue";
import { libraryApi, type Track } from "../services/library";
import { t } from "../i18n";

/** 文件夹视图（M4）：按来源目录浏览曲目，按路径排序呈现目录结构 */
const route = useRoute();
const router = useRouter();

interface FolderEntry {
  id: number;
  path: string;
}

const folders = ref<FolderEntry[]>([]);
const tracks = ref<Track[]>([]);
const loading = ref(false);

const activeId = computed(() => (route.params.id ? Number(route.params.id) : null));

function folderName(path: string) {
  return path.split(/[\\/]/).filter(Boolean).pop() ?? path;
}

async function loadFolders() {
  folders.value = await libraryApi.folderList();
  // 无选中或选中项已删除时落到第一个目录
  if (!folders.value.some((f) => f.id === activeId.value)) {
    const first = folders.value[0];
    if (first) {
      void router.replace({ name: "folders", params: { id: first.id } });
      return;
    }
  }
  await loadTracks();
}

async function loadTracks() {
  if (activeId.value == null) {
    tracks.value = [];
    return;
  }
  loading.value = true;
  try {
    tracks.value = await libraryApi.folderTracks(activeId.value);
  } finally {
    loading.value = false;
  }
}

onMounted(() => void loadFolders());
watch(activeId, () => void loadTracks());
</script>

<template>
  <section class="folder-view">
    <header class="toolbar">
      <h2>{{ t("folders.title") }}</h2>
      <span class="dim">{{ t("library.total", { n: tracks.length }) }}</span>
      <div class="spacer"></div>
      <PlayAllButton :tracks="tracks" />
    </header>
    <div class="panes">
      <nav class="folder-list">
        <button
          v-for="f in folders"
          :key="f.id"
          class="folder-item"
          :class="{ active: f.id === activeId }"
          :title="f.path"
          @click="router.push({ name: 'folders', params: { id: f.id } })"
        >
          <Folder :size="14" />
          <span class="f-name">{{ folderName(f.path) }}</span>
        </button>
        <div v-if="!folders.length" class="dim f-empty">{{ t("folders.empty") }}</div>
      </nav>
      <TrackTable
        class="table"
        :tracks="tracks"
        :empty-text="loading ? t('library.loading') : t('folders.emptyTracks')"
      />
    </div>
  </section>
</template>

<style scoped>
.folder-view {
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

.toolbar h2 {
  margin: 0;
  font-size: 18px;
}

.spacer {
  flex: 1;
}

.panes {
  flex: 1;
  min-height: 0;
  display: grid;
  grid-template-columns: 220px 1fr;
  gap: 12px;
}

.folder-list {
  overflow: auto;
  display: flex;
  flex-direction: column;
  gap: 2px;
  padding-right: 4px;
}

.folder-item {
  display: flex;
  align-items: center;
  gap: 8px;
  padding: 8px 10px;
  border: none;
  border-radius: 8px;
  background: transparent;
  color: var(--text);
  cursor: pointer;
  text-align: left;
  font-size: 13px;
}

.folder-item svg {
  display: block;
  color: var(--text-dim);
  flex-shrink: 0;
}

.folder-item:hover {
  background: var(--bg-hover);
}

.folder-item.active {
  background: var(--bg-hover);
  font-weight: 600;
}

.folder-item.active svg {
  color: var(--accent);
}

.f-name {
  min-width: 0;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.f-empty {
  padding: 8px 10px;
  font-size: 12px;
}
</style>
