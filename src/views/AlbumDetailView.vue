<script setup lang="ts">
import { onMounted, ref, watch } from "vue";
import { useRoute } from "vue-router";
import { Disc3 } from "lucide-vue-next";
import TrackTable from "../components/TrackTable.vue";
import { coverUrl, libraryApi, type AlbumInfo, type Track } from "../services/library";
import { t } from "../i18n";

const route = useRoute();
const album = ref<AlbumInfo | null>(null);
const tracks = ref<Track[]>([]);
const cover = ref<string | null>(null);

async function load(id: number) {
  album.value = null;
  tracks.value = [];
  const [albums, list] = await Promise.all([libraryApi.albumsQuery(), libraryApi.albumTracks(id)]);
  album.value = albums.find((a) => a.id === id) ?? null;
  tracks.value = list;
  cover.value = await coverUrl(album.value?.coverFile);
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
      </div>
    </header>
    <TrackTable :tracks="tracks" :empty-text="t('table.empty')" />
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
}

.head h2 {
  margin: 0 0 6px;
  font-size: 20px;
}
</style>
