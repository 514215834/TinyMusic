<script setup lang="ts">
import { onMounted, ref } from "vue";
import { useRouter } from "vue-router";
import { User } from "lucide-vue-next";
import { libraryApi, type ArtistInfo } from "../services/library";
import { t } from "../i18n";

const router = useRouter();
const artists = ref<ArtistInfo[]>([]);

onMounted(async () => {
  artists.value = await libraryApi.artistsQuery();
});

function open(artist: ArtistInfo) {
  void router.push({ name: "artist-detail", params: { id: artist.id } });
}
</script>

<template>
  <section class="artists">
    <header class="toolbar">
      <h2>{{ t("artists.title") }}</h2>
      <span class="dim">{{ artists.length }}</span>
    </header>
    <div v-if="artists.length" class="wall">
      <button v-for="a in artists" :key="a.id" class="card" @click="open(a)">
        <span class="avatar"><User :size="32" :stroke-width="1.5" /></span>
        <span class="name" :title="a.name">{{ a.name }}</span>
        <span class="sub dim"
          >{{ t("artists.albumCount", { n: a.albumCount }) }} ·
          {{ t("albums.trackCount", { n: a.trackCount }) }}</span
        >
      </button>
    </div>
    <div v-else class="empty dim">{{ t("artists.empty") }}</div>
  </section>
</template>

<style scoped>
.artists {
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

.wall {
  flex: 1;
  overflow: auto;
  display: grid;
  grid-template-columns: repeat(auto-fill, minmax(140px, 1fr));
  gap: 16px;
  align-content: start;
  padding: 4px;
}

.card {
  display: flex;
  flex-direction: column;
  align-items: center;
  gap: 6px;
  padding: 14px 10px;
  border: 1px solid var(--border);
  border-radius: 12px;
  background: var(--bg-elev);
  color: var(--text);
  cursor: pointer;
  text-align: center;
}

.card:hover {
  border-color: var(--accent);
}

.avatar {
  width: 72px;
  height: 72px;
  border-radius: 50%;
  background: var(--bg-hover);
  display: flex;
  align-items: center;
  justify-content: center;
  color: var(--text-dim);
}

.name {
  max-width: 100%;
  font-weight: 600;
  font-size: 13px;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.sub {
  font-size: 12px;
}

.empty {
  padding: 60px 20px;
  text-align: center;
}
</style>
