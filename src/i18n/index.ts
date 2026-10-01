import { ref } from "vue";

export type Locale = "zh" | "en";

/** 当前语言（默认中文，经设置持久化；t() 读取 ref，模板中自动响应切换） */
export const locale = ref<Locale>("zh");

type Dict = Record<string, string>;

const zh: Dict = {
  // 侧边栏
  "nav.library": "曲库",
  "nav.albums": "专辑",
  "nav.artists": "艺人",
  "nav.favorites": "收藏",
  "nav.settings": "设置",
  "sidebar.playlists": "歌单",
  "sidebar.addPlaylist": "新建歌单",
  "sidebar.renamePlaylist": "重命名",
  "sidebar.deletePlaylist": "删除歌单",
  "sidebar.emptyPlaylists": "暂无歌单，点击 + 新建",
  "sidebar.playlistName": "歌单名称",

  // 曲库
  "library.title": "曲库",
  "library.total": "共 {n} 首",
  "library.searchResult": "搜索到 {n} 首",
  "library.scanning": "扫描中 {done}/{total}",
  "library.rescan": "重新扫描",
  "library.import": "导入文件夹",
  "library.searchPlaceholder": "搜索（Ctrl+F）",
  "library.removeFolder": "移除该文件夹",
  "library.empty": "还没有曲目：点击右上角「导入文件夹」开始",
  "library.scanningEmpty": "正在扫描曲目…",
  "library.noMatch": "没有匹配的曲目",
  "library.loading": "加载中…",

  // 视图
  "albums.title": "专辑",
  "albums.empty": "还没有专辑：导入文件夹后自动按元数据归集",
  "albums.unknownArtist": "未知艺人",
  "albums.trackCount": "{n} 首",
  "artists.title": "艺人",
  "artists.empty": "还没有艺人：导入文件夹后自动按元数据归集",
  "artists.albumCount": "{n} 张专辑",
  "favorites.title": "收藏",
  "favorites.empty": "还没有收藏：在列表行悬停点亮红心，或按 L 收藏正在播放的曲目",
  "playlist.addTracks": "添加歌曲",
  "playlist.empty": "歌单还是空的：点击右上角「添加歌曲」",
  "playlist.remove": "从歌单移除",
  "table.title": "标题",
  "table.artist": "歌手",
  "table.album": "专辑",
  "table.duration": "时长",
  "table.empty": "暂无曲目",

  // 行内操作
  "track.favorite": "收藏",
  "track.unfavorite": "取消收藏",
  "track.playNext": "下一首播放",
  "track.addToQueue": "加入队列",

  // 播放器
  "player.notPlaying": "未在播放",
  "player.prev": "上一首",
  "player.next": "下一首",
  "player.play": "播放",
  "player.pause": "暂停",
  "player.mute": "静音",
  "player.unmute": "取消静音",
  "player.mode.sequential": "顺序播放",
  "player.mode.repeatAll": "列表循环",
  "player.mode.repeatOne": "单曲循环",
  "player.mode.shuffle": "随机播放",
  "player.queue": "播放队列",
  "player.queueEmpty": "队列为空：在列表行悬停选择「下一首播放」或「加入队列」",
  "player.queueClear": "清空",
  "player.removeFromQueue": "移出队列",
  "player.nowPlaying": "正在播放",
  "player.nowPlayingEmpty": "当前没有播放中的曲目",
  "player.noLyrics": "暂无歌词",

  // 迷你悬浮窗
  "overlay.close": "关闭迷你播放器",

  // 设置
  "settings.title": "设置",
  "settings.appearance": "外观",
  "settings.theme": "主题",
  "settings.theme.system": "跟随系统",
  "settings.theme.light": "浅色",
  "settings.theme.dark": "深色",
  "settings.language": "语言",
  "settings.miniPlayer": "迷你悬浮播放器",
  "settings.opacity": "悬停透明度",
  "settings.shortcut": "调整/锁定快捷键",
  "settings.shortcutHint": "如 Ctrl+Alt+M，回车应用",
  "settings.shortcutError": "注册失败：格式无效或已被其他程序占用",
  "settings.shortcutEmpty": "快捷键不能为空",
  "settings.about": "关于",
  "settings.aboutDesc": "TinyMusic · 极简本地音乐播放器",

  // 通用
  "common.close": "关闭",
  "common.search": "搜索",
  "common.none": "—",
};

const en: Dict = {
  "nav.library": "Library",
  "nav.albums": "Albums",
  "nav.artists": "Artists",
  "nav.favorites": "Favorites",
  "nav.settings": "Settings",
  "sidebar.playlists": "Playlists",
  "sidebar.addPlaylist": "New playlist",
  "sidebar.renamePlaylist": "Rename",
  "sidebar.deletePlaylist": "Delete playlist",
  "sidebar.emptyPlaylists": "No playlists yet, click + to create",
  "sidebar.playlistName": "Playlist name",

  "library.title": "Library",
  "library.total": "{n} tracks",
  "library.searchResult": "{n} results",
  "library.scanning": "Scanning {done}/{total}",
  "library.rescan": "Rescan",
  "library.import": "Import folder",
  "library.searchPlaceholder": "Search (Ctrl+F)",
  "library.removeFolder": "Remove this folder",
  "library.empty": "No tracks yet: click 'Import folder' to start",
  "library.scanningEmpty": "Scanning tracks…",
  "library.noMatch": "No matching tracks",
  "library.loading": "Loading…",

  "albums.title": "Albums",
  "albums.empty": "No albums yet: import a folder to collect by metadata",
  "albums.unknownArtist": "Unknown artist",
  "albums.trackCount": "{n} tracks",
  "artists.title": "Artists",
  "artists.empty": "No artists yet: import a folder to collect by metadata",
  "artists.albumCount": "{n} albums",
  "favorites.title": "Favorites",
  "favorites.empty": "No favorites yet: hover a row and click the heart, or press L",
  "playlist.addTracks": "Add tracks",
  "playlist.empty": "Playlist is empty: click 'Add tracks' to begin",
  "playlist.remove": "Remove from playlist",
  "table.title": "Title",
  "table.artist": "Artist",
  "table.album": "Album",
  "table.duration": "Duration",
  "table.empty": "No tracks",

  "track.favorite": "Favorite",
  "track.unfavorite": "Unfavorite",
  "track.playNext": "Play next",
  "track.addToQueue": "Add to queue",

  "player.notPlaying": "Not playing",
  "player.prev": "Previous",
  "player.next": "Next",
  "player.play": "Play",
  "player.pause": "Pause",
  "player.mute": "Mute",
  "player.unmute": "Unmute",
  "player.mode.sequential": "Sequential",
  "player.mode.repeatAll": "Repeat all",
  "player.mode.repeatOne": "Repeat one",
  "player.mode.shuffle": "Shuffle",
  "player.queue": "Play queue",
  "player.queueEmpty": "Queue is empty: hover a row and choose 'Play next' or 'Add to queue'",
  "player.queueClear": "Clear",
  "player.removeFromQueue": "Remove from queue",
  "player.nowPlaying": "Now Playing",
  "player.nowPlayingEmpty": "Nothing is playing right now",
  "player.noLyrics": "No lyrics available",

  "overlay.close": "Close mini player",

  "settings.title": "Settings",
  "settings.appearance": "Appearance",
  "settings.theme": "Theme",
  "settings.theme.system": "Follow system",
  "settings.theme.light": "Light",
  "settings.theme.dark": "Dark",
  "settings.language": "Language",
  "settings.miniPlayer": "Mini overlay player",
  "settings.opacity": "Hover opacity",
  "settings.shortcut": "Adjust/lock shortcut",
  "settings.shortcutHint": "e.g. Ctrl+Alt+M; press Enter to apply",
  "settings.shortcutError": "Registration failed: invalid shortcut or already in use",
  "settings.shortcutEmpty": "Shortcut cannot be empty",
  "settings.about": "About",
  "settings.aboutDesc": "TinyMusic · minimal local music player",

  "common.close": "Close",
  "common.search": "Search",
  "common.none": "—",
};

const DICTS: Record<Locale, Dict> = { zh, en };

export function setLocale(next: Locale) {
  locale.value = next;
}

/** 取翻译；带 {n} 占位符替换；缺 key 时回退中文再回退 key 本身 */
export function t(key: string, params?: Record<string, string | number>): string {
  const raw = DICTS[locale.value][key] ?? DICTS.zh[key] ?? key;
  if (!params) return raw;
  return Object.entries(params).reduce((acc, [k, v]) => acc.split(`{${k}}`).join(String(v)), raw);
}
