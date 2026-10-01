import { createRouter, createWebHistory } from "vue-router";
import AppShell from "../components/AppShell.vue";

const router = createRouter({
  history: createWebHistory(),
  routes: [
    {
      // 迷你悬浮窗（技术设计文档 §6）：独立 WebviewWindow 加载此路由，无应用外壳
      path: "/overlay",
      name: "overlay",
      component: () => import("../views/OverlayView.vue"),
    },
    {
      path: "/",
      component: AppShell,
      children: [
        { path: "", redirect: { name: "library" } },
        { path: "library", name: "library", component: () => import("../views/LibraryView.vue") },
        { path: "albums", name: "albums", component: () => import("../views/AlbumsView.vue") },
        {
          path: "albums/:id",
          name: "album-detail",
          component: () => import("../views/AlbumDetailView.vue"),
        },
        { path: "artists", name: "artists", component: () => import("../views/ArtistsView.vue") },
        {
          path: "artists/:id",
          name: "artist-detail",
          component: () => import("../views/ArtistDetailView.vue"),
        },
        {
          path: "folders/:id?",
          name: "folders",
          component: () => import("../views/FolderView.vue"),
        },
        {
          path: "favorites",
          name: "favorites",
          component: () => import("../views/FavoritesView.vue"),
        },
        {
          path: "history",
          name: "history",
          component: () => import("../views/HistoryView.vue"),
        },
        {
          path: "playlists/:id",
          name: "playlist",
          component: () => import("../views/PlaylistView.vue"),
        },
        {
          path: "smart/:id",
          name: "smart-playlist",
          component: () => import("../views/SmartPlaylistView.vue"),
        },
        {
          path: "duplicates",
          name: "duplicates",
          component: () => import("../views/DuplicatesView.vue"),
        },
        {
          path: "settings",
          name: "settings",
          component: () => import("../views/SettingsView.vue"),
        },
      ],
    },
  ],
});

export default router;
