<script lang="ts">
  let { options, selected = $bindable([]), onchange }: {
    options: { label: string; value: string }[];
    selected?: string[];
    onchange?: (v: string[]) => void;
  } = $props();

  function toggle(value: string) {
    const next = selected.includes(value)
      ? selected.filter(v => v !== value)
      : [...selected, value];
    selected = next;
    onchange?.(next);
  }
</script>

<div class="flex flex-wrap gap-2">
  {#each options as opt}
    <button
      onclick={() => toggle(opt.value)}
      class="px-3 py-1 rounded-full text-sm font-medium border transition-colors
             {selected.includes(opt.value)
               ? 'bg-indigo-600 text-white border-indigo-600'
               : 'bg-white text-gray-700 border-gray-300 hover:border-indigo-400'}"
    >
      {opt.label}
    </button>
  {/each}
</div>
