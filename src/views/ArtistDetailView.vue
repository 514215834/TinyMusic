<script setup lang="ts">
import { onMounted, ref, watch } from "vue";
import { useRoute } from "vue-router";
import { User } from "lucide-vue-next";
import TrackTable from "../components/TrackTable.vue";
import PlayAllButton from "../components/PlayAllButton.vue";
import { libraryApi, type ArtistInfo, type Track } from "../services/library";
import { t } from "../i18n";

const route = useRoute();
const artist = ref<ArtistInfo | null>(null);
const tracks = ref<Track[]>([]);

async function load(id: number) {
  artist.value = null;
  tracks.value = [];
  const [artists, list] = await Promise.all([
    libraryApi.artistsQuery(),
    libraryApi.artistTracks(id),
  ]);
  artist.value = artists.find((a) => a.id === id) ?? null;
  tracks.value = list;
}

onMounted(() => void load(Number(route.params.id)));
watch(
  () => route.params.id,
  (id) => {
    if (route.name === "artist-detail" && id) void load(Number(id));
  },
);
</script>

<template>
  <section class="artist-detail">
    <header class="head">
      <div class="avatar"><User :size="40" :stroke-width="1.5" /></div>
      <div>
        <h2>{{ artist?.name ?? "…" }}</h2>
        <div class="dim">
          {{
            artist
              ? `${t("artists.albumCount", { n: artist.albumCount })} · ${t("albums.trackCount", { n: artist.trackCount })}`
              : ""
          }}
        </div>
      </div>
      <div class="spacer"></div>
      <PlayAllButton :tracks="tracks" />
    </header>
    <TrackTable :tracks="tracks" :empty-text="t('table.empty')" />
  </section>
</template>

<style scoped>
.artist-detail {
  height: 100%;
  display: flex;
  flex-direction: column;
  gap: 12px;
}

.spacer {
  flex: 1;
}

.head {
  display: flex;
  align-items: center;
  gap: 16px;
}

.avatar {
  width: 84px;
  height: 84px;
  border-radius: 50%;
  background: var(--bg-hover);
  display: flex;
  align-items: center;
  justify-content: center;
  color: var(--text-dim);
  flex-shrink: 0;
}

.head h2 {
  margin: 0 0 6px;
  font-size: 20px;
}
</style>
