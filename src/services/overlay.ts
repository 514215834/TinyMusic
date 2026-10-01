import { commands, type SmtcState } from "../bindings";
import { unwrap } from "./ipc";

export type { SmtcState };

/** 迷你悬浮窗 / SMTC 命令封装（技术设计文档 §5 overlay_* / smtc_update） */
export const overlayApi = {
  toggle: async () => unwrap(await commands.overlayToggle()),
  hide: async () => unwrap(await commands.overlayHide()),
  setLocked: async (locked: boolean) => unwrap(await commands.overlaySetLocked(locked)),
  setOpacity: async (opacity: number) => unwrap(await commands.overlaySetOpacity(opacity)),
  setShortcut: async (shortcut: string) => unwrap(await commands.overlaySetShortcut(shortcut)),
  smtcUpdate: async (state: SmtcState) => unwrap(await commands.smtcUpdate(state)),
};
