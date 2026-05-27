import { invoke } from '@tauri-apps/api/core';
import type { Clip, ClipResult } from './types';

export const api = {
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

  shutdown: () =>
    invoke<void>('shutdown'),
};
