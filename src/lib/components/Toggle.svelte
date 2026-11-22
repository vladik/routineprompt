<script lang="ts">
  let { checked = $bindable(false), disabled = false, onchange }: {
    checked?: boolean;
    disabled?: boolean;
    onchange?: (v: boolean) => void;
  } = $props();
</script>

<button
  role="switch"
  aria-checked={checked}
  aria-label="Toggle"
  {disabled}
  onclick={() => { if (!disabled) { checked = !checked; onchange?.(checked); } }}
  class="relative inline-flex h-6 w-11 shrink-0 cursor-pointer rounded-full border-2 border-transparent
         transition-colors duration-200 focus:outline-none focus-visible:ring-2 focus-visible:ring-indigo-500
         {checked ? 'bg-indigo-600' : 'bg-gray-300'}
         {disabled ? 'opacity-50 cursor-not-allowed' : ''}"
>
  <span
    class="pointer-events-none inline-block h-5 w-5 rounded-full bg-white shadow ring-0
           transition duration-200 {checked ? 'translate-x-5' : 'translate-x-0'}"
  ></span>
</button>
