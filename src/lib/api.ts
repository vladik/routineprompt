import { invoke } from '@tauri-apps/api/core';
import { open } from '@tauri-apps/plugin-dialog';
import type { MediaAsset, Reminder, ReminderEvent } from './types';

export const api = {
  listReminders(): Promise<Reminder[]> {
    return invoke('list_reminders');
  },

  saveReminder(reminder: Reminder): Promise<void> {
    return invoke('save_reminder', { dto: reminder });
  },

  deleteReminder(id: string): Promise<void> {
    return invoke('delete_reminder', { id });
  },

  toggleReminder(id: string, enabled: boolean): Promise<void> {
    return invoke('toggle_reminder', { id, enabled });
  },

  listAssets(): Promise<MediaAsset[]> {
    return invoke('list_assets');
  },

  deleteAsset(id: string): Promise<number> {
    return invoke('delete_asset', { id });
  },

  uploadAsset(path: string, kind: 'image' | 'sound'): Promise<MediaAsset> {
    return invoke('upload_asset', { path, kind });
  },

  recentEvents(): Promise<ReminderEvent[]> {
    return invoke('recent_events');
  },

  acknowledgePopup(eventId: string | null, reminderId: string): Promise<void> {
    return invoke('acknowledge_popup', { eventId, reminderId });
  },

  getAssetsDir(): Promise<string> {
    return invoke('get_assets_dir');
  },

  async pickImage(): Promise<string | null> {
    const result = await open({
      multiple: false,
      filters: [{ name: 'Images', extensions: ['png', 'jpg', 'jpeg', 'gif', 'webp'] }],
    });
    return typeof result === 'string' ? result : null;
  },

  async pickSound(): Promise<string | null> {
    const result = await open({
      multiple: false,
      filters: [{ name: 'Sounds', extensions: ['mp3', 'wav', 'm4a', 'ogg', 'aac'] }],
    });
    return typeof result === 'string' ? result : null;
  },
};
