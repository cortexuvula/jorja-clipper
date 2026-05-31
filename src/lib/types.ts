export interface ClipResult {
  success: boolean;
  path: string;
  start_time: number;
  end_time: number;
  error?: string;
}

export interface Clip {
  id: number;
  video_path: string;
  clip_path: string;
  start_time: number;
  end_time: number;
  created_at: string;
}

export type Theme = 'dark' | 'light';

export interface Settings {
  buffer_before: number;
  buffer_after: number;
  clip_key: string;
  output_dir?: string;
  theme: Theme;
}
