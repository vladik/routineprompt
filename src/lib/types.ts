export type DisplayMode = 'gentle' | 'persistent' | 'focus';
export type SchedType = 'daily' | 'weekly' | 'once';
export type AssetKind = 'image' | 'sound';

export interface Reminder {
  id: string;
  title: string;
  message: string;
  image_asset_ids: string[];
  sound_asset_id: string | null;
  sched_type: SchedType;
  sched_time: string;        // "HH:MM" for daily/weekly; RFC3339 for once
  sched_days: string | null; // "0,1,4" weekday indices (Mon=0)
  display_mode: DisplayMode;
  enabled: boolean;
  refire_after_mins: number | null;
  refire_until_ack: boolean;
  min_dismissal_mins: number | null;
  created_at: string;
  updated_at: string;
}

export interface MediaAsset {
  id: string;
  name: string;
  kind: AssetKind;
  filename: string;
  created_at: string;
}

export interface ReminderEvent {
  title: string;
  fired_at: string;
  acknowledged_at: string | null;
}

export interface PopupPayload {
  reminder: Reminder;
  event_id: string | null;
  more_pending: number;
  assets_dir: string;
  assets: MediaAsset[];
}

export function emptyReminder(): Reminder {
  const now = new Date();
  const hh = String(now.getHours()).padStart(2, '0');
  const mm = String(now.getMinutes()).padStart(2, '0');
  return {
    id: crypto.randomUUID(),
    title: '',
    message: '',
    image_asset_ids: [],
    sound_asset_id: null,
    sched_type: 'daily',
    sched_time: `${hh}:${mm}`,
    sched_days: null,
    display_mode: 'persistent',
    enabled: true,
    refire_after_mins: null,
    refire_until_ack: false,
    min_dismissal_mins: null,
    created_at: new Date().toISOString(),
    updated_at: new Date().toISOString(),
  };
}
