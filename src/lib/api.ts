import { invoke } from '@tauri-apps/api/core';
import type { Clip, ClipResult } from './types';

export interface OpenVideoResponse {
  play_path: string;
  source_path: string;
  duration: number;
  converted: boolean;
}

export const api = {
  openVideo: (path: string) =>
    invoke<OpenVideoResponse>('open_video', { path }),

  saveClip: (currentPos: number, duration: number) =>
    invoke<ClipResult>('save_clip', { currentPos, duration }),

  getClips: () =>
    invoke<Clip[]>('get_clips'),

  deleteClip: (id: number, clipPath: string) =>
    invoke<void>('delete_clip', { id, clipPath }),

  startVideoServer: (path: string) =>
    invoke<string>('start_video_server', { path }),
};
