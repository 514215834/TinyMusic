<script setup lang="ts">
import { computed, onMounted, ref, watch } from "vue";
import { useRoute } from "vue-router";
import { Plus, Sparkles, Trash2 } from "lucide-vue-next";
import TrackTable from "../components/TrackTable.vue";
import PlayAllButton from "../components/PlayAllButton.vue";
import { smartApi, type SmartRule, type Track } from "../services/smartPlaylists";
import { t } from "../i18n";

/** 智能歌单（M4）：规则式实时生成，只读列表（不可拖拽排序/增删曲目） */

type Field = "playCount" | "lastPlayed" | "genre" | "year" | "addedAt";
type Op = "gte" | "lte" | "eq" | "contains" | "withinDays";

const FIELD_OPS: Record<Field, Op[]> = {
  playCount: ["gte", "lte"],
  lastPlayed: ["withinDays"],
  genre: ["contains", "eq"],
  year: ["gte", "lte"],
  addedAt: ["withinDays"],
};
const FIELDS = Object.keys(FIELD_OPS) as Field[];

const route = useRoute();
const listId = computed(() => Number(route.params.id));

const name = ref("");
const rules = ref<SmartRule[]>([]);
const trackLimit = ref("");
const tracks = ref<Track[]>([]);
const error = ref("");
const saving = ref(false);
const loadedId = ref<number | null>(null);

let saveTimer: ReturnType<typeof setTimeout> | undefined;
let suppressWatch = false;

async function load() {
  error.value = "";
  const lists = await smartApi.list();
  const current = lists.find((p) => p.id === listId.value);
  if (!current) {
    error.value = t("smart.notFound");
    return;
  }
  suppressWatch = true;
  name.value = current.name;
  rules.value = current.rules.map((r) => ({ ...r }));
  trackLimit.value = current.trackLimit?.toString() ?? "";
  suppressWatch = false;
  loadedId.value = current.id;
  await refreshTracks();
}

async function refreshTracks() {
  if (loadedId.value !== listId.value) return;
  try {
    tracks.value = await smartApi.tracks(listId.value);
  } catch (e) {
    error.value = String(e instanceof Error ? e.message : e);
  }
}

/** 规则/名称/上限变更 → 防抖保存 → 列表实时刷新（规则改动即时生效） */
watch([rules, name, trackLimit], () => {
  if (suppressWatch || loadedId.value !== listId.value) return;
  clearTimeout(saveTimer);
  saveTimer = setTimeout(() => void save(), 500);
});
watch(listId, () => void load());

async function save() {
  if (loadedId.value == null) return;
  saving.value = true;
  error.value = "";
  try {
    const limitRaw = trackLimit.value.trim();
    const limit = limitRaw ? Math.max(1, Math.floor(Number(limitRaw))) : null;
    // 数字输入的空串归一为 null（后端 Option<f64> 语义）
    const normalized = rules.value.map((r) => ({
      ...r,
      num: typeof r.num === "number" && Number.isFinite(r.num) ? r.num : null,
      text: typeof r.text === "string" && r.text.trim() ? r.text : null,
    }));
    await smartApi.update(listId.value, name.value, normalized, limit);
    await refreshTracks();
  } catch (e) {
    error.value = String(e instanceof Error ? e.message : e);
  } finally {
    saving.value = false;
  }
}

function addRule() {
  const f: Field = "playCount";
  rules.value.push({ field: f, op: FIELD_OPS[f][0]!, num: null, text: null });
}

function removeRule(i: number) {
  rules.value.splice(i, 1);
}

function onFieldChange(rule: SmartRule) {
  rule.op = FIELD_OPS[rule.field][0]!;
  rule.num = null;
  rule.text = null;
}

const isNumeric = (rule: SmartRule) => rule.op !== "contains" && rule.op !== "eq";

const fieldLabel = (f: string) => t(`smart.field.${f}`);
const opLabel = (o: string) => t(`smart.op.${o}`);
const opOptions = (rule: SmartRule) => FIELD_OPS[rule.field];

onMounted(() => void load());
</script>

<template>
  <section class="smart-view">
    <header class="toolbar">
      <Sparkles :size="18" />
      <h2>{{ name || "…" }}</h2>
      <span class="dim">{{ t("albums.trackCount", { n: tracks.length }) }}</span>
      <span v-if="saving" class="dim">{{ t("smart.saving") }}</span>
      <div class="spacer"></div>
      <span v-if="error" class="error">{{ error }}</span>
      <PlayAllButton :tracks="tracks" />
    </header>

    <div class="editor">
      <label class="name-field">
        <span class="dim">{{ t("sidebar.playlistName") }}</span>
        <input v-model="name" spellcheck="false" />
      </label>

      <div class="rules">
        <div v-for="(rule, i) in rules" :key="i" class="rule">
          <select v-model="rule.field" @change="onFieldChange(rule)">
            <option v-for="f in FIELDS" :key="f" :value="f">{{ fieldLabel(f) }}</option>
          </select>
          <select v-model="rule.op">
            <option v-for="o in opOptions(rule)" :key="o" :value="o">{{ opLabel(o) }}</option>
          </select>
          <input
            v-if="isNumeric(rule)"
            v-model="rule.num"
            class="val"
            type="number"
            min="0"
            :placeholder="t('smart.valueNum')"
          />
          <input v-else v-model="rule.text" class="val" spellcheck="false" :placeholder="t('smart.valueText')" />
          <button class="rm" :title="t('smart.removeRule')" @click="removeRule(i)">
            <Trash2 :size="13" />
          </button>
        </div>
        <div v-if="!rules.length" class="dim no-rules">{{ t("smart.noRules") }}</div>
        <button class="add-rule" @click="addRule">
          <Plus :size="13" />
          {{ t("smart.addRule") }}
        </button>
      </div>

      <label class="limit-field">
        <span class="dim">{{ t("smart.limit") }}</span>
        <input v-model="trackLimit" type="number" min="1" max="1000" :placeholder="t('smart.noLimit')" />
      </label>
    </div>

    <TrackTable
      class="table"
      :tracks="tracks"
      :empty-text="t('smart.empty')"
    />
  </section>
</template>

<style scoped>
.smart-view {
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

.error {
  color: #ef4444;
  font-size: 12px;
}

.editor {
  border: 1px solid var(--border);
  border-radius: 12px;
  padding: 12px 14px;
  display: flex;
  flex-direction: column;
  gap: 10px;
  background: var(--bg-elev);
}

.name-field,
.limit-field {
  display: flex;
  align-items: center;
  gap: 8px;
}

.name-field span,
.limit-field span {
  font-size: 12px;
  flex: none;
}

.name-field input {
  max-width: 280px;
}

.limit-field input {
  width: 90px;
}

.editor input {
  padding: 6px 10px;
  border: 1px solid var(--border);
  border-radius: 8px;
  background: var(--bg);
  color: var(--text);
  font-size: 13px;
  outline: none;
  min-width: 0;
}

.editor input:focus {
  border-color: var(--accent);
}

.rules {
  display: flex;
  flex-direction: column;
  gap: 6px;
}

.rule {
  display: flex;
  align-items: center;
  gap: 8px;
}

.rule select {
  padding: 6px 8px;
  border: 1px solid var(--border);
  border-radius: 8px;
  background: var(--bg);
  color: var(--text);
  font-size: 13px;
  outline: none;
}

.rule .val {
  flex: 1;
  max-width: 260px;
}

.rm {
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

.rm:hover {
  background: var(--bg-hover);
  color: #ef4444;
}

.rm svg {
  display: block;
}

.no-rules {
  font-size: 12px;
  padding: 2px 0;
}

.add-rule {
  align-self: flex-start;
  display: inline-flex;
  align-items: center;
  gap: 5px;
  padding: 5px 12px;
  border: 1px dashed var(--border);
  border-radius: 8px;
  background: transparent;
  color: var(--text-dim);
  cursor: pointer;
  font-size: 12px;
}

.add-rule svg {
  display: block;
}

.add-rule:hover {
  color: var(--text);
  border-color: var(--accent);
}
</style>
