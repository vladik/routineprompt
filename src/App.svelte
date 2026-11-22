<script lang="ts">
  import { onMount } from 'svelte';
  import { convertFileSrc } from '@tauri-apps/api/core';
  import { api } from './lib/api';
  import { emptyReminder, type MediaAsset, type Reminder, type ReminderEvent } from './lib/types';
  import Toggle from './lib/components/Toggle.svelte';
  import PillGroup from './lib/components/PillGroup.svelte';
  import TimeSpinner from './lib/components/TimeSpinner.svelte';
  import SectionLabel from './lib/components/SectionLabel.svelte';

  type Screen = 'list' | 'form' | 'analytics' | 'media' | 'picker';

  let screen = $state<Screen>('list');
  let reminders = $state<Reminder[]>([]);
  let assets = $state<MediaAsset[]>([]);
  let events = $state<ReminderEvent[]>([]);
  let assetsDir = $state('');
  let form = $state<Reminder>(emptyReminder());
  let isNew = $state(true);
  let mainEl = $state<HTMLElement | undefined>(undefined);

  $effect(() => {
    // Reset scroll position on every screen change
    screen;
    mainEl?.scrollTo({ top: 0, behavior: 'instant' });
  });

  // picker state
  let pickerMode = $state<'image' | 'sound'>('image');
  let pickerImageIdx = $state<number | null>(null); // null = append, number = replace at index
  let pickerImages = $derived(assets.filter(a => a.kind === 'image'));

  const WEEKDAYS = [
    { label: 'Mon', value: '0' }, { label: 'Tue', value: '1' },
    { label: 'Wed', value: '2' }, { label: 'Thu', value: '3' },
    { label: 'Fri', value: '4' }, { label: 'Sat', value: '5' },
    { label: 'Sun', value: '6' },
  ];

  const DISPLAY_MODES = [
    { label: 'Gentle', value: 'gentle' },
    { label: 'Persistent', value: 'persistent' },
    { label: 'Focus', value: 'focus' },
  ];

  onMount(async () => {
    [reminders, assets, events, assetsDir] = await Promise.all([
      api.listReminders(),
      api.listAssets(),
      api.recentEvents(),
      api.getAssetsDir(),
    ]);
  });

  async function saveAndBack() {
    form.updated_at = new Date().toISOString();
    await api.saveReminder(form);
    reminders = await api.listReminders();
    screen = 'list';
  }

  async function deleteReminder(id: string) {
    await api.deleteReminder(id);
    reminders = reminders.filter(r => r.id !== id);
  }

  async function toggle(r: Reminder) {
    await api.toggleReminder(r.id, !r.enabled);
    r.enabled = !r.enabled;
  }

  function editReminder(r: Reminder) {
    form = JSON.parse(JSON.stringify($state.snapshot(r))) as Reminder;
    isNew = false;
    screen = 'form';
  }

  function newReminder() {
    form = emptyReminder();
    isNew = true;
    screen = 'form';
  }

  let confirmDeleteId = $state<string | null>(null);

  function handleDeleteClick(id: string, usageCount: number) {
    if (usageCount > 0) {
      confirmDeleteId = id;
    } else {
      doDeleteConfirmed(id);
    }
  }

  async function doDeleteConfirmed(id: string) {
    confirmDeleteId = null;
    const affected = await api.deleteAsset(id);
    assets = assets.filter(a => a.id !== id);
    if (affected > 0) {
      reminders = await api.listReminders();
    }
  }

  async function uploadFromPicker(kind: 'image' | 'sound') {
    const path = kind === 'image' ? await api.pickImage() : await api.pickSound();
    if (!path) return;
    const asset = await api.uploadAsset(path, kind);
    assets = [...assets, asset];
    return asset;
  }

  // Upload a new image from disk and immediately append it to the form list
  async function uploadAndSelectImage() {
    const asset = await uploadFromPicker('image');
    if (!asset) return;
    form.image_asset_ids = [...form.image_asset_ids, asset.id];
  }

  function openImagePicker(idx: number | null) {
    pickerMode = 'image';
    pickerImageIdx = idx;
    screen = 'picker';
  }

  function openSoundPicker() {
    pickerMode = 'sound';
    screen = 'picker';
  }

  function selectAsset(id: string) {
    if (pickerMode === 'image') {
      const ids = [...form.image_asset_ids];
      if (pickerImageIdx !== null) {
        ids[pickerImageIdx] = id;
      } else {
        ids.push(id);
      }
      form.image_asset_ids = ids;
    } else {
      form.sound_asset_id = id;
    }
    screen = 'form';
  }

  function moveImage(i: number, delta: number) {
    const ids = [...form.image_asset_ids];
    const j = i + delta;
    if (j < 0 || j >= ids.length) return;
    [ids[i], ids[j]] = [ids[j], ids[i]];
    form.image_asset_ids = ids;
  }

  function removeImage(i: number) {
    const ids = [...form.image_asset_ids];
    ids.splice(i, 1);
    form.image_asset_ids = ids;
  }

  function schedDaysArray(form: Reminder): string[] {
    return form.sched_days ? form.sched_days.split(',').filter(Boolean) : [];
  }

  function setSchedDays(vals: string[]) {
    form.sched_days = vals.join(',') || null;
  }

  function schedLabel(r: Reminder): string {
    const time = r.sched_time.length > 5 ? '...' : fmtTime(r.sched_time);
    if (r.sched_type === 'daily') return `Daily at ${time}`;
    if (r.sched_type === 'once') return `Once at ${time}`;
    const days = (r.sched_days ?? '').split(',')
      .filter(Boolean)
      .map(n => WEEKDAYS[parseInt(n)]?.label ?? '')
      .join(', ');
    return `${days || 'No days'} at ${time}`;
  }

  function fmtTime(hhmm: string): string {
    const [h, m] = hhmm.split(':').map(Number);
    const ampm = h < 12 ? 'AM' : 'PM';
    const h12 = h % 12 === 0 ? 12 : h % 12;
    return `${h12}:${String(m).padStart(2,'0')} ${ampm}`;
  }

  function fmtDate(iso: string): string {
    return new Date(iso).toLocaleString(undefined, { month:'short', day:'numeric', hour:'numeric', minute:'2-digit' });
  }
</script>

<!-- ── NAV ── -->
<div class="flex flex-col h-screen overflow-hidden bg-gray-50 select-none">

  <!-- Header -->
  <header class="bg-white border-b border-gray-200 px-4 py-3 flex items-center justify-between shrink-0">
    {#if screen === 'list'}
      <span class="font-bold text-indigo-600 text-lg">RoutinePrompt</span>
      <div class="flex gap-2">
        <button onclick={() => { screen = 'analytics'; events = []; api.recentEvents().then(e => events = e); }}
          class="text-sm text-gray-500 hover:text-indigo-600 px-2 py-1 rounded hover:bg-gray-100">Analytics</button>
        <button onclick={() => { screen = 'media'; api.listAssets().then(a => assets = a); }}
          class="text-sm text-gray-500 hover:text-indigo-600 px-2 py-1 rounded hover:bg-gray-100">Media</button>
        <button onclick={newReminder}
          class="bg-indigo-600 text-white text-sm px-3 py-1 rounded-lg hover:bg-indigo-700">+ Add</button>
      </div>
    {:else if screen === 'form'}
      <button onclick={() => screen = 'list'} class="text-indigo-600 text-sm hover:underline">← Back</button>
      <span class="font-semibold text-gray-800">{isNew ? 'New Reminder' : 'Edit Reminder'}</span>
      <button onclick={saveAndBack} class="bg-indigo-600 text-white text-sm px-3 py-1 rounded-lg hover:bg-indigo-700">Save</button>
    {:else if screen === 'picker'}
      <button onclick={() => screen = 'form'} class="text-indigo-600 text-sm hover:underline">← Back</button>
      <span class="font-semibold text-gray-800">{pickerMode === 'image' ? 'Pick Image' : 'Pick Sound'}</span>
      <div class="w-16"></div>
    {:else}
      <button onclick={() => screen = 'list'} class="text-indigo-600 text-sm hover:underline">← Back</button>
      <span class="font-semibold text-gray-800">{screen === 'analytics' ? 'Analytics' : 'Media Library'}</span>
      <div class="w-16"></div>
    {/if}
  </header>

  <!-- Body -->
  <main bind:this={mainEl} class="flex-1 overflow-y-auto">

    <!-- ── LIST SCREEN ── -->
    {#if screen === 'list'}
      {#if reminders.length === 0}
        <div class="flex flex-col items-center justify-center h-full text-gray-400 gap-3">
          <span class="text-5xl">🔔</span>
          <p class="text-sm">No reminders yet</p>
          <button onclick={newReminder} class="bg-indigo-600 text-white text-sm px-4 py-2 rounded-lg hover:bg-indigo-700">
            Create your first reminder
          </button>
        </div>
      {:else}
        <ul class="divide-y divide-gray-100">
          {#each reminders as r (r.id)}
            <li class="flex items-center px-4 py-3 gap-3 bg-white hover:bg-gray-50 transition-colors">
              <div class="flex-1 min-w-0">
                <p class="font-medium text-gray-900 truncate">{r.title}</p>
                <p class="text-xs text-gray-500">{schedLabel(r)}</p>
              </div>
              <Toggle checked={r.enabled} onchange={() => toggle(r)} />
              <button onclick={() => editReminder(r)}
                class="text-gray-400 hover:text-indigo-600 text-sm px-2">Edit</button>
              <button onclick={() => deleteReminder(r.id)}
                class="text-gray-400 hover:text-red-500 text-sm px-2">✕</button>
            </li>
          {/each}
        </ul>
      {/if}

    <!-- ── FORM SCREEN ── -->
    {:else if screen === 'form'}
      <div class="px-4 py-4 space-y-4 max-w-md mx-auto">

        <SectionLabel label="Title" />
        <input
          bind:value={form.title}
          placeholder="e.g. Take medication"
          class="w-full border border-gray-300 rounded-lg px-3 py-2 text-sm focus:outline-none focus:ring-2 focus:ring-indigo-400"
        />

        <SectionLabel label="Message (optional)" />
        <textarea
          bind:value={form.message}
          rows="2"
          placeholder="Additional details..."
          class="w-full border border-gray-300 rounded-lg px-3 py-2 text-sm focus:outline-none focus:ring-2 focus:ring-indigo-400 resize-none"
        ></textarea>

        <SectionLabel label="Schedule" />
        <div class="flex gap-2">
          {#each ['daily','weekly','once'] as t}
            <button
              onclick={() => { form.sched_type = t as typeof form.sched_type; }}
              class="flex-1 py-1.5 text-sm rounded-lg border transition-colors
                     {form.sched_type === t
                       ? 'bg-indigo-600 text-white border-indigo-600'
                       : 'bg-white text-gray-700 border-gray-300 hover:border-indigo-400'}"
            >{t.charAt(0).toUpperCase() + t.slice(1)}</button>
          {/each}
        </div>

        {#if form.sched_type === 'weekly'}
          <PillGroup
            options={WEEKDAYS}
            selected={schedDaysArray(form)}
            onchange={setSchedDays}
          />
        {/if}

        <TimeSpinner bind:value={form.sched_time} />

        <SectionLabel label="Alert style" />
        <div class="flex gap-2">
          {#each DISPLAY_MODES as m}
            <button
              onclick={() => form.display_mode = m.value as typeof form.display_mode}
              class="flex-1 py-1.5 text-sm rounded-lg border transition-colors
                     {form.display_mode === m.value
                       ? 'bg-indigo-600 text-white border-indigo-600'
                       : 'bg-white text-gray-700 border-gray-300 hover:border-indigo-400'}"
            >{m.label}</button>
          {/each}
        </div>

        <SectionLabel label="Images (up to 3)" />
        <div class="space-y-2">
          {#each form.image_asset_ids as id, i (i)}
            {@const asset = assets.find(a => a.id === id)}
            <div class="flex items-center gap-2 bg-white border border-gray-200 rounded-lg px-2 py-1.5">
              {#if asset}
                <img
                  src={convertFileSrc(`${assetsDir}/${asset.filename}`)}
                  alt={asset.name}
                  class="w-9 h-9 object-cover rounded flex-shrink-0 bg-gray-100"
                />
                <span class="text-sm text-gray-700 flex-1 truncate">{asset.name}</span>
              {:else}
                <div class="w-9 h-9 rounded bg-gray-100 flex items-center justify-center flex-shrink-0">
                  <span class="text-gray-400 text-xs">?</span>
                </div>
                <span class="text-sm text-gray-400 flex-1">Missing</span>
              {/if}
              <div class="flex items-center gap-0.5 flex-shrink-0">
                <button onclick={() => moveImage(i, -1)} disabled={i === 0}
                  class="p-1 text-gray-400 hover:text-indigo-600 disabled:opacity-25">↑</button>
                <button onclick={() => moveImage(i, 1)} disabled={i === form.image_asset_ids.length - 1}
                  class="p-1 text-gray-400 hover:text-indigo-600 disabled:opacity-25">↓</button>
                <button onclick={() => openImagePicker(i)}
                  class="p-1 text-gray-400 hover:text-indigo-600 text-xs" title="Replace">↺</button>
                <button onclick={() => removeImage(i)}
                  class="p-1 text-red-400 hover:text-red-600">✕</button>
              </div>
            </div>
          {/each}
          {#if form.image_asset_ids.length < 3}
            <button onclick={() => openImagePicker(null)}
              class="w-full py-2 text-sm border border-dashed border-gray-300 text-gray-500
                     rounded-lg hover:border-indigo-400 hover:text-indigo-600 transition-colors">
              + Add image
            </button>
          {/if}
        </div>

        <SectionLabel label="Sound (optional)" />
        <div class="flex items-center gap-3">
          {#if form.sound_asset_id}
            {@const a = assets.find(x => x.id === form.sound_asset_id)}
            <span class="text-sm text-gray-700 flex-1 truncate">{a?.name ?? '...'}</span>
            <button onclick={() => form.sound_asset_id = null}
              class="text-xs text-red-500 hover:underline">Remove</button>
          {:else}
            <button onclick={openSoundPicker}
              class="text-sm text-indigo-600 hover:underline">Pick sound…</button>
          {/if}
        </div>

        <SectionLabel label="Behavior" />
        <div class="space-y-3">
          <label class="flex items-center justify-between">
            <span class="text-sm text-gray-700">Re-fire if not acknowledged</span>
            <Toggle bind:checked={form.refire_until_ack} />
          </label>
          {#if form.refire_until_ack}
            <label class="flex items-center justify-between">
              <span class="text-sm text-gray-700">Re-fire after (minutes)</span>
              <input
                type="number" min="1" max="60"
                value={form.refire_after_mins ?? 5}
                oninput={e => form.refire_after_mins = parseInt((e.target as HTMLInputElement).value) || null}
                class="w-16 border border-gray-300 rounded px-2 py-1 text-sm text-right"
              />
            </label>
          {/if}
          {#if form.display_mode !== 'gentle'}
            <label class="flex items-center justify-between">
              <span class="text-sm text-gray-700">Minimum dismissal (seconds)</span>
              <input
                type="number" min="0" max="120"
                value={form.min_dismissal_mins ?? 0}
                oninput={e => form.min_dismissal_mins = parseInt((e.target as HTMLInputElement).value) || null}
                class="w-16 border border-gray-300 rounded px-2 py-1 text-sm text-right"
              />
            </label>
          {/if}
        </div>

        <SectionLabel label="Enabled" />
        <Toggle bind:checked={form.enabled} />

        <div class="pb-6"></div>
      </div>

    <!-- ── ANALYTICS SCREEN ── -->
    {:else if screen === 'analytics'}
      {#if events.length === 0}
        <div class="flex items-center justify-center h-full text-gray-400 text-sm">No events yet</div>
      {:else}
        <ul class="divide-y divide-gray-100">
          {#each events as ev}
            <li class="px-4 py-3 bg-white">
              <div class="flex items-center justify-between">
                <span class="font-medium text-gray-800 text-sm">{ev.title}</span>
                {#if ev.acknowledged_at}
                  <span class="text-xs text-green-600 bg-green-50 px-2 py-0.5 rounded-full">Acknowledged</span>
                {:else}
                  <span class="text-xs text-orange-500 bg-orange-50 px-2 py-0.5 rounded-full">Dismissed</span>
                {/if}
              </div>
              <p class="text-xs text-gray-400 mt-0.5">Fired: {fmtDate(ev.fired_at)}</p>
              {#if ev.acknowledged_at}
                <p class="text-xs text-gray-400">Acked: {fmtDate(ev.acknowledged_at)}</p>
              {/if}
            </li>
          {/each}
        </ul>
      {/if}

    <!-- ── MEDIA LIBRARY SCREEN ── -->
    {:else if screen === 'media'}
      <div class="px-4 py-4">
        <div class="flex gap-2 mb-4">
          <button onclick={() => uploadFromPicker('image')}
            class="flex-1 py-2 text-sm border border-indigo-300 text-indigo-600 rounded-lg hover:bg-indigo-50">
            + Upload Image
          </button>
          <button onclick={() => uploadFromPicker('sound')}
            class="flex-1 py-2 text-sm border border-indigo-300 text-indigo-600 rounded-lg hover:bg-indigo-50">
            + Upload Sound
          </button>
        </div>
        {#if assets.length === 0}
          <p class="text-center text-gray-400 text-sm mt-8">No media uploaded yet</p>
        {:else}
          <div class="grid grid-cols-3 gap-3">
            {#each assets as a (a.id)}
              {@const usageCount = reminders.filter(r =>
                r.image_asset_ids.includes(a.id) || r.sound_asset_id === a.id
              ).length}
              <div class="relative group">
                {#if a.kind === 'image'}
                  <img
                    src={convertFileSrc(`${assetsDir}/${a.filename}`)}
                    alt={a.name}
                    class="w-full aspect-square object-cover rounded-lg bg-gray-100"
                  />
                {:else}
                  <div class="w-full aspect-square rounded-lg bg-indigo-50 flex flex-col items-center justify-center gap-1">
                    <span class="text-2xl">🎵</span>
                    <span class="text-xs text-gray-600 text-center px-1 truncate w-full">{a.name}</span>
                  </div>
                {/if}
                {#if confirmDeleteId === a.id}
                  <div class="absolute inset-0 bg-white/95 rounded-lg flex flex-col items-center justify-center gap-2 p-2">
                    <p class="text-xs text-center text-gray-700 leading-tight">
                      Used by {usageCount} reminder{usageCount === 1 ? '' : 's'}<br/>Delete anyway?
                    </p>
                    <div class="flex gap-1">
                      <button onclick={() => confirmDeleteId = null}
                        class="text-xs px-2 py-1 border border-gray-300 rounded hover:bg-gray-50">Cancel</button>
                      <button onclick={() => doDeleteConfirmed(a.id)}
                        class="text-xs px-2 py-1 bg-red-500 text-white rounded hover:bg-red-600">Delete</button>
                    </div>
                  </div>
                {:else}
                  <button
                    onclick={() => handleDeleteClick(a.id, usageCount)}
                    class="absolute top-1 right-1 bg-red-500 text-white text-xs rounded px-1
                           opacity-0 group-hover:opacity-100 transition-opacity"
                  >✕</button>
                {/if}
              </div>
            {/each}
          </div>
        {/if}
      </div>

    <!-- ── PICKER SCREEN ── -->
    {:else if screen === 'picker'}
      <div class="px-4 py-4">
        {#if pickerMode === 'image'}
          <!-- Upload from disk → auto-adds to reminder and goes back -->
          <button onclick={uploadAndSelectImage}
            class="w-full py-2 text-sm border border-indigo-300 text-indigo-600 rounded-lg hover:bg-indigo-50 mb-4">
            + Upload from disk
          </button>
          {#if pickerImages.length === 0}
            <p class="text-center text-gray-400 text-sm">No images in library yet</p>
          {:else}
            <p class="text-xs text-gray-400 mb-2">Or pick from library:</p>
            <div class="grid grid-cols-3 gap-3">
              {#each pickerImages as a (a.id)}
                <button onclick={() => selectAsset(a.id)}
                  class="rounded-lg overflow-hidden hover:ring-2 hover:ring-indigo-500 focus:outline-none">
                  <img
                    src={convertFileSrc(`${assetsDir}/${a.filename}`)}
                    alt={a.name}
                    class="w-full aspect-square object-cover bg-gray-100"
                  />
                </button>
              {/each}
            </div>
          {/if}
        {:else}
          <!-- Sound picker -->
          <button onclick={() => uploadFromPicker('sound')}
            class="w-full py-2 text-sm border border-indigo-300 text-indigo-600 rounded-lg hover:bg-indigo-50 mb-4">
            + Upload from disk
          </button>
          {#if assets.filter(a => a.kind === 'sound').length === 0}
            <p class="text-center text-gray-400 text-sm">No sounds in library yet</p>
          {:else}
            <p class="text-xs text-gray-400 mb-2">Or pick from library:</p>
            <ul class="divide-y divide-gray-100">
              {#each assets.filter(a => a.kind === 'sound') as a (a.id)}
                <li>
                  <button onclick={() => selectAsset(a.id)}
                    class="w-full flex items-center gap-3 px-2 py-3 hover:bg-indigo-50 rounded-lg text-left">
                    <span class="text-xl">🎵</span>
                    <span class="text-sm text-gray-800">{a.name}</span>
                  </button>
                </li>
              {/each}
            </ul>
          {/if}
        {/if}
      </div>
    {/if}

  </main>
</div>
