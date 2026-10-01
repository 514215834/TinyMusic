<script setup lang="ts">
import { onMounted, ref } from "vue";
import TrackTable from "../components/TrackTable.vue";
import { useFavoritesStore } from "../stores/favorites";
import { libraryApi, type Track } from "../services/library";
import { t } from "../i18n";

const favorites = useFavoritesStore();
const tracks = ref<Track[]>([]);

onMounted(async () => {
  await favorites.load();
  tracks.value = await libraryApi.favoritesList();
});
</script>

<template>
  <section class="favorites">
    <header class="toolbar">
      <h2>{{ t("favorites.title") }}</h2>
      <span class="dim">{{ t("albums.trackCount", { n: tracks.length }) }}</span>
    </header>
    <TrackTable :tracks="tracks" :empty-text="t('favorites.empty')" />
  </section>
</template>

<style scoped>
.favorites {
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
</style>
