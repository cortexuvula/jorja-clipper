import { writable } from 'svelte/store';

export interface PlayerState {
  videoLoaded: boolean;
  isPlaying: boolean;
  currentTime: number;
  duration: number;
  videoPath: string | null;
}

const initialState: PlayerState = {
  videoLoaded: false,
  isPlaying: false,
  currentTime: 0,
  duration: 0,
  videoPath: null,
};

export const playerStore = writable<PlayerState>(initialState);
