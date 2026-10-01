<script setup lang="ts">
import { onMounted, ref, watch } from "vue";
import { useRoute } from "vue-router";
import { Disc3, Globe } from "lucide-vue-next";
import TrackTable from "../components/TrackTable.vue";
import ScrapeModal from "../components/ScrapeModal.vue";
import PlayAllButton from "../components/PlayAllButton.vue";
import { coverUrl, libraryApi, type AlbumInfo, type Track } from "../services/library";
import { t } from "../i18n";

const route = useRoute();
const album = ref<AlbumInfo | null>(null);
const tracks = ref<Track[]>([]);
const cover = ref<string | null>(null);
const scraping = ref(false);

async function refresh(id: number) {
  const [albums, list] = await Promise.all([libraryApi.albumsQuery(), libraryApi.albumTracks(id)]);
  album.value = albums.find((a) => a.id === id) ?? null;
  tracks.value = list;
  cover.value = await coverUrl(album.value?.coverFile);
}

/** 首次进入/切换专辑：先清空避免展示上一专辑的残留数据 */
async function load(id: number) {
  album.value = null;
  tracks.value = [];
  await refresh(id);
}

onMounted(() => void load(Number(route.params.id)));
watch(
  () => route.params.id,
  (id) => {
    if (route.name === "album-detail" && id) void load(Number(id));
  },
);
</script>

<template>
  <section class="album-detail">
    <header class="head">
      <div class="cover">
        <img v-if="cover" :src="cover" alt="" />
        <Disc3 v-else :size="48" :stroke-width="1.5" />
      </div>
      <div class="meta">
        <h2>{{ album?.name ?? "…" }}</h2>
        <div class="dim">
          {{ album?.artist || t("albums.unknownArtist") }}
          <template v-if="album?.year"> · {{ album.year }}</template>
          · {{ t("albums.trackCount", { n: tracks.length }) }}
        </div>
        <div class="actions">
          <PlayAllButton :tracks="tracks" />
          <button v-if="album" class="scrape-btn" @click="scraping = true">
            <Globe :size="13" />
            {{ t("scrape.albumAction") }}
          </button>
        </div>
      </div>
    </header>
    <TrackTable :tracks="tracks" :empty-text="t('table.empty')" />

    <Teleport to="body">
      <ScrapeModal
        v-if="scraping && album"
        mode="album"
        :target-id="album.id"
        :name="album.name"
        :artist="album.artist"
        @close="scraping = false"
        @applied="() => void refresh(album!.id)"
      />
    </Teleport>
  </section>
</template>

<style scoped>
.album-detail {
  height: 100%;
  display: flex;
  flex-direction: column;
  gap: 12px;
}

.head {
  display: flex;
  align-items: center;
  gap: 16px;
}

.cover {
  width: 96px;
  height: 96px;
  border-radius: 12px;
  background: var(--bg-hover);
  display: flex;
  align-items: center;
  justify-content: center;
  color: var(--text-dim);
  overflow: hidden;
  flex-shrink: 0;
}

.cover img {
  width: 100%;
  height: 100%;
  object-fit: cover;
  display: block;
  transition: transform 0.3s ease;
}

.cover:hover img {
  transform: scale(1.04);
}

.head h2 {
  margin: 0 0 6px;
  font-size: 20px;
}

.actions {
  display: flex;
  align-items: center;
  gap: 8px;
  margin-top: 12px;
}

.scrape-btn {
  display: inline-flex;
  align-items: center;
  gap: 6px;
  padding: 6px 14px;
  border: 1px solid var(--border);
  border-radius: 999px;
  background: var(--bg-elev);
  color: var(--text);
  cursor: pointer;
  font-size: 13px;
}

.scrape-btn svg {
  display: block;
}

.scrape-btn:hover {
  border-color: var(--accent);
  color: var(--accent);
}
</style>
