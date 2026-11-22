<script lang="ts">
  import { onMount } from 'svelte';
  import { invoke, convertFileSrc } from '@tauri-apps/api/core';
  import { listen } from '@tauri-apps/api/event';
  import { getCurrentWindow } from '@tauri-apps/api/window';
  import type { PopupPayload } from './lib/types';

  let payload        = $state<PopupPayload | null>(null);
  let dismissed      = $state(false);
  let canDismiss     = $state(false);
  let countdown      = $state<number | null>(null);
  let initialCount   = $state<number | null>(null);
  let imageIndex     = $state(0);
  let countdownTimer: ReturnType<typeof setInterval> | null = null;

  function clearTimer() {
    if (countdownTimer) { clearInterval(countdownTimer); countdownTimer = null; }
  }

  function resetState() {
    clearTimer();
    dismissed    = false;
    canDismiss   = false;
    countdown    = null;
    initialCount = null;
    imageIndex   = 0;
  }

  function setupTimer(r: PopupPayload['reminder']) {
    const mode = r.display_mode;

    if (mode === 'gentle') {
      // 5-second auto-dismiss countdown; button available immediately
      initialCount = 5;
      countdown    = 5;
      canDismiss   = true;
      countdownTimer = setInterval(() => {
        countdown = (countdown ?? 1) - 1;
        if (countdown <= 0) {
          clearTimer();
          countdown = null;
          dismiss(false);
        }
      }, 1000);

    } else if (mode === 'focus' && r.min_dismissal_mins) {
      // Mandatory lock — button unavailable until timer expires
      const secs   = Math.round(r.min_dismissal_mins * 60);
      initialCount = secs;
      countdown    = secs;
      canDismiss   = false;
      countdownTimer = setInterval(() => {
        countdown = (countdown ?? 1) - 1;
        if (countdown <= 0) {
          clearTimer();
          countdown  = null;
          canDismiss = true;
        }
      }, 1000);

    } else {
      // persistent, or focus with no lock set
      canDismiss = true;
    }
  }

  onMount(async () => {
    return listen<PopupPayload>('popup-trigger', (event) => {
      resetState();
      payload = event.payload;
      setupTimer(event.payload.reminder);
    });
  });

  async function dismiss(acknowledged: boolean) {
    if (dismissed) return;
    dismissed = true;
    clearTimer();
    if (payload) {
      await invoke('acknowledge_popup', {
        eventId: acknowledged ? (payload.event_id ?? null) : null,
        reminderId: payload.reminder.id,
      });
    }
    payload = null;
    await getCurrentWindow().hide();
  }

  let imageIds       = $derived(payload?.reminder.image_asset_ids ?? []);
  let currentImgSrc  = $derived((() => {
    if (!payload || imageIds.length === 0) return null;
    const asset = payload.assets.find(a => a.id === imageIds[imageIndex]);
    return asset ? convertFileSrc(`${payload.assets_dir}/${asset.filename}`) : null;
  })());

  function prevImg() { imageIndex = (imageIndex - 1 + imageIds.length) % imageIds.length; }
  function nextImg() { imageIndex = (imageIndex + 1) % imageIds.length; }

  let progressPct = $derived(
    countdown !== null && initialCount ? Math.round((countdown / initialCount) * 100) : 0
  );
</script>

<div class="fixed inset-0 flex flex-col bg-gray-950 text-white overflow-hidden select-none">
  {#if payload}
    {@const r = payload.reminder}

    <!-- Full-bleed background: image or gradient -->
    {#if currentImgSrc}
      <img src={currentImgSrc} alt=""
           class="absolute inset-0 w-full h-full object-cover pointer-events-none" />
      <div class="absolute inset-0 bg-gradient-to-t from-black via-black/50 to-black/10 pointer-events-none"></div>
    {:else}
      <div class="absolute inset-0 bg-gradient-to-br from-indigo-950 via-gray-950 to-gray-900 pointer-events-none"></div>
    {/if}

    <!-- Image nav arrows -->
    {#if imageIds.length > 1}
      <button onclick={prevImg}
        class="absolute left-6 top-1/2 -translate-y-1/2 z-20 w-14 h-14 rounded-full
               bg-black/40 hover:bg-black/70 backdrop-blur-sm flex items-center justify-center
               text-3xl transition-colors">‹</button>
      <button onclick={nextImg}
        class="absolute right-6 top-1/2 -translate-y-1/2 z-20 w-14 h-14 rounded-full
               bg-black/40 hover:bg-black/70 backdrop-blur-sm flex items-center justify-center
               text-3xl transition-colors">›</button>
    {/if}

    <!-- Content anchored to bottom -->
    <div class="relative z-10 mt-auto px-10 pb-12 flex flex-col gap-6 max-w-3xl">

      <!-- Dot indicators -->
      {#if imageIds.length > 1}
        <div class="flex gap-2">
          {#each imageIds as _, i}
            <button onclick={() => imageIndex = i}
              class="w-2.5 h-2.5 rounded-full transition-colors
                     {i === imageIndex ? 'bg-white' : 'bg-white/35'}">
            </button>
          {/each}
        </div>
      {/if}

      <!-- Title + message -->
      <div class="space-y-3">
        <h1 class="text-5xl font-bold leading-tight drop-shadow-lg">{r.title}</h1>
        {#if r.message}
          <p class="text-white/80 text-xl leading-relaxed drop-shadow">{r.message}</p>
        {/if}
        {#if payload.more_pending > 0}
          <p class="text-sm text-indigo-300">
            +{payload.more_pending} more reminder{payload.more_pending === 1 ? '' : 's'} queued
          </p>
        {/if}
      </div>

      <!-- Gentle: auto-dismiss progress bar -->
      {#if r.display_mode === 'gentle' && countdown !== null}
        <div class="w-full bg-white/20 rounded-full h-1 overflow-hidden">
          <div class="bg-white h-1 rounded-full transition-[width] duration-1000 ease-linear"
               style="width: {progressPct}%"></div>
        </div>
      {/if}

      <!-- Action buttons -->
      <div class="flex gap-4">
        {#if r.display_mode === 'gentle'}
          <button onclick={() => dismiss(false)}
            class="px-10 py-4 rounded-2xl text-base font-medium border border-white/25
                   text-white/70 hover:bg-white/10 backdrop-blur-sm transition-colors">
            Skip
          </button>
        {/if}
        <button onclick={() => dismiss(true)}
          disabled={!canDismiss}
          class="flex-1 py-4 rounded-2xl text-lg font-semibold transition-colors
                 {canDismiss
                   ? 'bg-indigo-600 hover:bg-indigo-500'
                   : 'bg-white/10 text-white/50 cursor-not-allowed'}">
          {!canDismiss && countdown !== null ? `Wait ${countdown}s…` : '✓ Done'}
        </button>
      </div>

    </div>
  {/if}
</div>
