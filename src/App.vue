<script setup lang="ts">
import { onMounted } from "vue";
import { useRoute } from "vue-router";
import router from "./router";
import { usePlayerStore } from "./stores/player";

// 应用根组件：纯路由出口。
// 主窗口三区布局由 AppShell 承载；迷你悬浮窗(/overlay)独立渲染，无应用外壳。
const route = useRoute();
const player = usePlayerStore();

onMounted(async () => {
  // 迷你窗是纯薄镜像（§6.4）：不初始化播放引擎，避免第二 audio 实例与重复 SMTC
  await router.isReady();
  if (route.name !== "overlay") void player.init();
});
</script>

<template>
  <RouterView />
</template>
