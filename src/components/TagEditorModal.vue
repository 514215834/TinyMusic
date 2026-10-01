<script setup lang="ts">
import { reactive, ref } from "vue";
import { Globe, X } from "lucide-vue-next";
import { libraryApi, type Track } from "../services/library";
import { t } from "../i18n";

const props = defineProps<{ track: Track }>();
const emit = defineEmits<{ close: []; saved: [track: Track]; scrape: [] }>();

/** 表单初值取自当前曲目；空白文本提交为 null（清除该字段） */
const form = reactive({
  title: props.track.title,
  artist: props.track.artist ?? "",
  album: props.track.album ?? "",
  genre: props.track.genre ?? "",
  year: props.track.year?.toString() ?? "",
  trackNo: props.track.trackNo?.toString() ?? "",
});
const saving = ref(false);
const error = ref("");

function numOrNull(v: string): number | null {
  const s = v.trim();
  if (!s) return null;
  const n = Number(s);
  return Number.isInteger(n) && n >= 0 ? n : NaN;
}

async function save() {
  error.value = "";
  const year = numOrNull(form.year);
  const trackNo = numOrNull(form.trackNo);
  if (Number.isNaN(year) || Number.isNaN(trackNo)) {
    error.value = t("tags.invalidNumber");
    return;
  }
  if (!form.title.trim()) {
    error.value = t("tags.titleRequired");
    return;
  }
  const clean = (s: string) => {
    const v = s.trim();
    return v ? v : null;
  };
  saving.value = true;
  try {
    const updated = await libraryApi.tagUpdate(props.track.id, {
      title: form.title.trim(),
      artist: clean(form.artist),
      album: clean(form.album),
      genre: clean(form.genre),
      year,
      trackNo,
    });
    emit("saved", updated);
    emit("close");
  } catch (e) {
    error.value = String(e instanceof Error ? e.message : e);
  } finally {
    saving.value = false;
  }
}
</script>

<template>
  <div class="modal-mask" @click.self="emit('close')">
    <div class="modal">
      <header class="m-head">
        <h3>{{ t("tags.title") }}</h3>
        <button class="m-close" :title="t('common.close')" @click="emit('close')">
          <X :size="14" />
        </button>
      </header>
      <form class="m-body" @submit.prevent="save">
        <label class="field">
          <span>{{ t("table.title") }}</span>
          <input v-model="form.title" spellcheck="false" required />
        </label>
        <label class="field">
          <span>{{ t("table.artist") }}</span>
          <input v-model="form.artist" spellcheck="false" />
        </label>
        <label class="field">
          <span>{{ t("table.album") }}</span>
          <input v-model="form.album" spellcheck="false" />
        </label>
        <div class="row">
          <label class="field">
            <span>{{ t("tags.genre") }}</span>
            <input v-model="form.genre" spellcheck="false" />
          </label>
          <label class="field">
            <span>{{ t("tags.year") }}</span>
            <input v-model="form.year" inputmode="numeric" spellcheck="false" />
          </label>
          <label class="field">
            <span>{{ t("tags.trackNo") }}</span>
            <input v-model="form.trackNo" inputmode="numeric" spellcheck="false" />
          </label>
        </div>
        <p class="hint dim">{{ t("tags.hint") }}</p>
        <p v-if="error" class="error">{{ error }}</p>
        <footer class="m-foot">
          <button type="button" class="ghost scrape-btn" @click="emit('scrape')">
            <Globe :size="13" />
            {{ t("scrape.rowAction") }}
          </button>
          <span class="spacer"></span>
          <button type="button" class="ghost" @click="emit('close')">
            {{ t("common.close") }}
          </button>
          <button type="submit" class="primary" :disabled="saving">
            {{ saving ? t("tags.saving") : t("tags.save") }}
          </button>
        </footer>
      </form>
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
  z-index: 120;
  animation: fade-in 0.15s ease both;
}

.modal {
  animation: pop-in 0.2s cubic-bezier(0.2, 0.9, 0.3, 1.15) both;
  width: 420px;
  max-width: calc(100vw - 48px);
  background: var(--bg-elev);
  border: 1px solid var(--border);
  border-radius: 12px;
  overflow: hidden;
}

.m-head {
  display: flex;
  align-items: center;
  justify-content: space-between;
  padding: 12px 16px;
  border-bottom: 1px solid var(--border);
}

.m-head h3 {
  margin: 0;
  font-size: 15px;
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

.m-body {
  padding: 16px;
  display: flex;
  flex-direction: column;
  gap: 12px;
}

.field {
  display: flex;
  flex-direction: column;
  gap: 4px;
  flex: 1;
  min-width: 0;
}

.field span {
  font-size: 12px;
  color: var(--text-dim);
}

.field input {
  padding: 7px 10px;
  border: 1px solid var(--border);
  border-radius: 8px;
  background: var(--bg);
  color: var(--text);
  font-size: 13px;
  outline: none;
  min-width: 0;
}

.field input:focus {
  border-color: var(--accent);
}

.row {
  display: flex;
  gap: 10px;
}

.hint {
  margin: 0;
  font-size: 12px;
}

.error {
  margin: 0;
  color: #ef4444;
  font-size: 12px;
}

.m-foot {
  display: flex;
  align-items: center;
  justify-content: flex-end;
  gap: 8px;
}

.m-foot .spacer {
  flex: 1;
}

.scrape-btn svg {
  display: block;
}

button {
  padding: 7px 16px;
  border-radius: 8px;
  cursor: pointer;
  font-size: 13px;
}

button.primary {
  border: 1px solid var(--accent);
  background: var(--accent);
  color: var(--on-accent);
}

button.primary:disabled {
  opacity: 0.6;
  cursor: default;
}

button.ghost {
  border: 1px solid var(--border);
  background: var(--bg-elev);
  color: var(--text);
}

button.ghost:hover {
  background: var(--bg-hover);
}
</style>
