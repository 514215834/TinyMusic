/** 播放控制全局快捷键动作（M7）：与后端 shortcuts::ACTIONS 对应 */
export type ShortcutAction = "toggle" | "prev" | "next" | "mini";

import { commands } from "../bindings";
import { unwrap } from "./ipc";

/** 设置/清除全局快捷键：null = 清除绑定；失败抛错且旧键保持可用 */
export const globalShortcutsApi = {
  set: async (action: ShortcutAction, shortcut: string | null) =>
    unwrap(await commands.shortcutSet(action, shortcut)),
};
