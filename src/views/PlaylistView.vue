<script setup lang="ts">
import { computed, onMounted, ref, watch } from "vue";
import { useRoute } from "vue-router";
import { save } from "@tauri-apps/plugin-dialog";
import { Download, ListMusic, Plus } from "lucide-vue-next";
import AddTracksModal from "../components/AddTracksModal.vue";
import TrackTable from "../components/TrackTable.vue";
import PlayAllButton from "../components/PlayAllButton.vue";
import { playlistsApi, type Playlist, type Track } from "../services/playlists";
import { t } from "../i18n";

const route = useRoute();
const playlist = ref<Playlist | null>(null);
const tracks = ref<Track[]>([]);
const showAdd = ref(false);
const exportError = ref("");

const existingIds = computed(() => tracks.value.map((t) => t.id));

async function load(id: number) {
  playlist.value = null;
  tracks.value = [];
  const [lists, list] = await Promise.all([playlistsApi.list(), playlistsApi.tracks(id)]);
  playlist.value = lists.find((p) => p.id === id) ?? null;
  tracks.value = list;
}

onMounted(() => void load(Number(route.params.id)));
watch(
  () => route.params.id,
  (id) => {
    if (route.name === "playlist" && id) void load(Number(id));
  },
);

async function addToPlaylist(track: Track) {
  if (!playlist.value) return;
  await playlistsApi.addTracks(playlist.value.id, [track.id]);
  tracks.value = await playlistsApi.tracks(playlist.value.id);
}

async function removeFromPlaylist(track: Track) {
  if (!playlist.value) return;
  await playlistsApi.removeTrack(playlist.value.id, track.id);
  tracks.value = tracks.value.filter((t) => t.id !== track.id);
}

/** 拖拽重排：本地先动，再提交新顺序 */
async function reorder(from: number, to: number) {
  if (!playlist.value) return;
  const ids = tracks.value.map((t) => t.id);
  const [moved] = ids.splice(from, 1);
  if (moved != null) ids.splice(to, 0, moved);
  tracks.value = ids
    .map((id) => tracks.value.find((t) => t.id === id))
    .filter((t): t is Track => !!t);
  await playlistsApi.reorder(playlist.value.id, ids);
}

/* M3：导出为 M3U8（save 对话框选择目标路径） */
async function exportPlaylist() {
  if (!playlist.value) return;
  exportError.value = "";
  const path = await save({
    defaultPath: `${playlist.value.name}.m3u8`,
    filters: [{ name: "M3U8", extensions: ["m3u8"] }],
  });
  if (typeof path !== "string") return;
  try {
    await playlistsApi.exportM3u8(playlist.value.id, path);
  } catch (e) {
    exportError.value = String(e instanceof Error ? e.message : e);
  }
}
</script>

<template>
  <section class="playlist-view">
    <header class="toolbar">
      <ListMusic :size="18" />
      <h2>{{ playlist?.name ?? "…" }}</h2>
      <span class="dim">{{ t("albums.trackCount", { n: tracks.length }) }}</span>
      <div class="spacer"></div>
      <span v-if="exportError" class="error">{{ exportError }}</span>
      <PlayAllButton :tracks="tracks" />
      <button class="ghost" @click="exportPlaylist">
        <Download :size="14" />
        {{ t("playlist.export") }}
      </button>
      <button class="add" @click="showAdd = true">
        <Plus :size="14" />
        {{ t("playlist.addTracks") }}
      </button>
    </header>
    <TrackTable
      :tracks="tracks"
      playlist-mode
      draggable
      :empty-text="t('playlist.empty')"
      @remove="removeFromPlaylist"
      @reorder="reorder"
    />

    <AddTracksModal
      v-if="showAdd"
      :playlist-id="playlist?.id ?? 0"
      :existing-ids="existingIds"
      @close="showAdd = false"
      @add="addToPlaylist"
    />
  </section>
</template>

<style scoped>
.playlist-view {
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

.add {
  padding: 6px 14px;
  border: 1px solid var(--accent);
  border-radius: 8px;
  background: var(--accent);
  color: var(--on-accent);
  cursor: pointer;
  display: inline-flex;
  align-items: center;
  gap: 6px;
}

.add svg {
  display: block;
}

.ghost {
  padding: 6px 12px;
  border: 1px solid var(--border);
  border-radius: 8px;
  background: var(--bg-elev);
  color: var(--text);
  cursor: pointer;
  display: inline-flex;
  align-items: center;
  gap: 6px;
  font-size: 13px;
}

.ghost svg {
  display: block;
}

.ghost:hover {
  background: var(--bg-hover);
}

.error {
  color: #ef4444;
  font-size: 12px;
}
</style>
