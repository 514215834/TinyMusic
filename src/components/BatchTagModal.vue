<script setup lang="ts">
import { computed, reactive, ref } from "vue";
import { X } from "lucide-vue-next";
import { libraryApi, type Track } from "../services/library";
import { t } from "../i18n";

/**
 * 批量标签编辑（M6）：统一设置 艺人/专辑/流派/年份，留空 = 不修改；
 * 曲号按传入顺序自动重编。写回源文件（lofty）并同步曲库，单首失败不中断。
 */
const props = defineProps<{ tracks: Track[] }>();
const emit = defineEmits<{ close: []; saved: [updated: Track[]] }>();

const form = reactive({
  artist: "",
  album: "",
  genre: "",
  year: "",
  renumber: false,
  trackNoStart: 1,
});
const saving = ref(false);
const error = ref("");
const failed = ref(0);
const firstError = ref("");

const count = computed(() => props.tracks.length);

function numOrNull(v: string): number | null | "invalid" {
  const s = v.trim();
  if (!s) return null;
  const n = Number(s);
  return Number.isInteger(n) && n >= 0 ? n : "invalid";
}

async function save() {
  error.value = "";
  failed.value = 0;
  firstError.value = "";
  const year = numOrNull(form.year);
  if (year === "invalid") {
    error.value = t("tags.invalidNumber");
    return;
  }
  const clean = (s: string) => {
    const v = s.trim();
    return v ? v : null;
  };
  if (form.renumber && (!Number.isInteger(form.trackNoStart) || form.trackNoStart < 0)) {
    error.value = t("tags.invalidNumber");
    return;
  }
  saving.value = true;
  try {
    // 顺序 = 当前列表顺序（重编曲号按此位次）
    const ids = props.tracks.map((tr) => tr.id);
    const result = await libraryApi.tagUpdateBatch(ids, {
      artist: clean(form.artist),
      album: clean(form.album),
      genre: clean(form.genre),
      year,
      trackNoStart: form.renumber ? form.trackNoStart : null,
    });
    // 就地更新行对象（列表共享同一响应式引用，所有视图同步刷新）
    for (const updated of result.updated) {
      const local = props.tracks.find((tr) => tr.id === updated.id);
      if (local) Object.assign(local, updated);
    }
    failed.value = result.failed;
    firstError.value = result.firstError ?? "";
    if (result.failed === 0) emit("close");
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
        <h3>{{ t("batch.title", { n: count }) }}</h3>
        <button class="m-close" :title="t('common.close')" @click="emit('close')">
          <X :size="14" />
        </button>
      </header>
      <form class="m-body" @submit.prevent="save">
        <p class="dim hint">{{ t("batch.hint") }}</p>
        <div class="row">
          <label class="field">
            <span>{{ t("table.artist") }}</span>
            <input v-model="form.artist" :placeholder="t('batch.unchanged')" spellcheck="false" />
          </label>
          <label class="field">
            <span>{{ t("table.album") }}</span>
            <input v-model="form.album" :placeholder="t('batch.unchanged')" spellcheck="false" />
          </label>
        </div>
        <div class="row">
          <label class="field">
            <span>{{ t("tags.genre") }}</span>
            <input v-model="form.genre" :placeholder="t('batch.unchanged')" spellcheck="false" />
          </label>
          <label class="field">
            <span>{{ t("tags.year") }}</span>
            <input v-model="form.year" inputmode="numeric" :placeholder="t('batch.unchanged')" spellcheck="false" />
          </label>
        </div>
        <div class="row renumber">
          <label class="check">
            <input v-model="form.renumber" type="checkbox" />
            <span>{{ t("batch.renumber") }}</span>
          </label>
          <label v-if="form.renumber" class="field start">
            <span>{{ t("batch.startFrom") }}</span>
            <input v-model.number="form.trackNoStart" type="number" min="0" />
          </label>
        </div>
        <p v-if="error" class="error">{{ error }}</p>
        <p v-if="failed > 0" class="error">
          {{ t("batch.failed", { n: failed }) }}
          <template v-if="firstError">（{{ firstError }}）</template>
        </p>
        <footer class="m-foot">
          <button type="button" class="ghost" @click="emit('close')">
            {{ t("common.close") }}
          </button>
          <button type="submit" class="primary" :disabled="saving">
            {{ saving ? t("tags.saving") : t("batch.apply", { n: count }) }}
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
  z-index: 130;
  animation: fade-in 0.15s ease both;
}

.modal {
  animation: pop-in 0.2s cubic-bezier(0.2, 0.9, 0.3, 1.15) both;
  width: 440px;
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

.hint {
  margin: 0;
  font-size: 12px;
}

.row {
  display: flex;
  gap: 10px;
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

.field input::placeholder {
  color: var(--text-dim);
  opacity: 0.6;
}

.renumber {
  align-items: center;
}

.check {
  display: inline-flex;
  align-items: center;
  gap: 6px;
  font-size: 13px;
  cursor: pointer;
}

.check input {
  accent-color: var(--accent);
}

.start {
  flex: 0 0 120px;
}

.error {
  margin: 0;
  color: #ef4444;
  font-size: 12px;
  word-break: break-all;
}

.m-foot {
  display: flex;
  justify-content: flex-end;
  gap: 8px;
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
