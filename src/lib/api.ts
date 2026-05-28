import { invoke } from '@tauri-apps/api/core';
import type { Clip, ClipResult } from './types';

export const api = {
  createMpvWindow: () =>
    invoke<number>('create_mpv_window'),

  positionMpvWindow: (x: number, y: number, width: number, height: number) =>
    invoke<void>('position_mpv_window', { x, y, width, height }),

  setMpvVisible: (visible: boolean) =>
    invoke<void>('set_mpv_visible', { visible }),

  openVideo: (path: string, wid?: number) =>
    invoke<number>('open_video', { path, wid }),

  togglePause: () =>
    invoke<void>('toggle_pause'),

  seek: (seconds: number) =>
    invoke<void>('seek', { seconds }),

  getPosition: () =>
    invoke<number>('get_position'),

  saveClip: () =>
    invoke<ClipResult>('save_clip'),

  getClips: () =>
    invoke<Clip[]>('get_clips'),

  deleteClip: (id: number, clipPath: string) =>
    invoke<void>('delete_clip', { id, clipPath }),

  shutdown: () =>
    invoke<void>('shutdown'),
};
