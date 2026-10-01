<script setup lang="ts">
import { computed, onMounted, ref } from "vue";
import { useRouter } from "vue-router";
import { Disc3, ImageDown } from "lucide-vue-next";
import CoverBatchWizard from "../components/CoverBatchWizard.vue";
import { coverUrl, libraryApi, type AlbumInfo } from "../services/library";
import { t } from "../i18n";

const router = useRouter();
const albums = ref<AlbumInfo[]>([]);
const covers = ref<Record<number, string | null>>({});
const wizardOpen = ref(false);
const missingCount = computed(() => albums.value.filter((a) => !a.coverFile).length);

async function reload() {
  albums.value = await libraryApi.albumsQuery();
  // 封面就绪后逐张替换占位（列表渲染不阻塞在封面 IO 上）
  for (const a of albums.value) {
    covers.value[a.id] = await coverUrl(a.coverFile);
  }
}

onMounted(reload);

function open(album: AlbumInfo) {
  void router.push({ name: "album-detail", params: { id: album.id } });
}
</script>

<template>
  <section class="albums">
    <header class="toolbar">
      <h2>{{ t("albums.title") }}</h2>
      <span class="dim">{{ albums.length }}</span>
      <div class="spacer"></div>
      <button class="wizard-btn" :disabled="!missingCount" :title="t('coverWiz.missing', { n: missingCount })" @click="wizardOpen = true">
        <ImageDown :size="13" />
        {{ t("coverWiz.open") }}
        <span v-if="missingCount" class="badge">{{ missingCount }}</span>
      </button>
    </header>
    <div v-if="albums.length" class="wall">
      <button v-for="a in albums" :key="a.id" class="card" @click="open(a)">
        <span class="cover">
          <img v-if="covers[a.id]" :src="covers[a.id] ?? undefined" alt="" />
          <Disc3 v-else :size="36" :stroke-width="1.5" />
        </span>
        <span class="name" :title="a.name">{{ a.name }}</span>
        <span class="sub dim"
          >{{ a.artist || t("albums.unknownArtist") }} ·
          {{ t("albums.trackCount", { n: a.trackCount }) }}</span
        >
      </button>
    </div>
    <div v-else class="empty dim">{{ t("albums.empty") }}</div>

    <CoverBatchWizard v-if="wizardOpen" @close="wizardOpen = false" @done="reload" />
  </section>
</template>

<style scoped>
.albums {
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

.wizard-btn {
  display: inline-flex;
  align-items: center;
  gap: 6px;
  padding: 6px 14px;
  border: 1px solid var(--border);
  border-radius: 999px;
  background: var(--bg-elev);
  color: var(--text);
  cursor: pointer;
  font-size: 12px;
}

.wizard-btn svg {
  display: block;
}

.wizard-btn:hover:not(:disabled) {
  border-color: var(--accent);
  color: var(--accent);
}

.wizard-btn:disabled {
  opacity: 0.5;
  cursor: default;
}

.wizard-btn .badge {
  padding: 1px 8px;
  border-radius: 999px;
  background: color-mix(in srgb, var(--accent) 16%, transparent);
  color: var(--accent);
  font-weight: 600;
}

.wall {
  flex: 1;
  min-height: 0;
  overflow: auto;
  display: grid;
  grid-template-columns: repeat(auto-fill, minmax(150px, 1fr));
  gap: 16px;
  align-content: start;
  /* 底部让位固定播放栏：最后一行可完整滚出；滚动条占位恒定防进入时抖动 */
  padding: 4px 4px 88px;
  scrollbar-gutter: stable;
}

.card {
  display: flex;
  flex-direction: column;
  align-items: flex-start;
  gap: 6px;
  padding: 10px;
  border: 1px solid var(--border);
  border-radius: 12px;
  background: var(--bg-elev);
  color: var(--text);
  cursor: pointer;
  text-align: left;
}

.card:hover {
  border-color: var(--accent);
}

.cover {
  width: 100%;
  aspect-ratio: 1;
  border-radius: 8px;
  background: var(--bg-hover);
  display: flex;
  align-items: center;
  justify-content: center;
  color: var(--text-dim);
  overflow: hidden;
}

.cover img {
  width: 100%;
  height: 100%;
  object-fit: cover;
  display: block;
}

.name {
  width: 100%;
  font-weight: 600;
  font-size: 13px;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.sub {
  width: 100%;
  font-size: 12px;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.empty {
  padding: 60px 20px;
  text-align: center;
}
</style>
