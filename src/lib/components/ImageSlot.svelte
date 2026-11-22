<script lang="ts">
  import { convertFileSrc } from '@tauri-apps/api/core';

  let { assetId, assetsDir, assets, onpick, onremove }: {
    assetId: string | null;
    assetsDir: string;
    assets: import('../types').MediaAsset[];
    onpick: () => void;
    onremove: () => void;
  } = $props();

  let asset = $derived(assets.find(a => a.id === assetId) ?? null);
  let src   = $derived(asset ? convertFileSrc(`${assetsDir}/${asset.filename}`) : null);
</script>

<div class="relative w-24 h-24 rounded-lg border-2 border-dashed border-gray-300
            flex items-center justify-center bg-gray-50 overflow-hidden group">
  {#if src}
    <img {src} alt={asset?.name} class="w-full h-full object-cover" />
    <button
      onclick={onremove}
      class="absolute top-0 right-0 bg-red-500 text-white text-xs rounded-bl px-1
             opacity-0 group-hover:opacity-100 transition-opacity"
    >✕</button>
  {:else}
    <button
      onclick={onpick}
      class="flex flex-col items-center gap-1 text-gray-400 hover:text-indigo-500 transition-colors"
    >
      <span class="text-2xl">+</span>
      <span class="text-xs">Image</span>
    </button>
  {/if}
</div>
