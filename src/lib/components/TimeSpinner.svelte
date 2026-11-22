<script lang="ts">
  // Accepts and emits "HH:MM" (24-hour). Displays with AM/PM and allows direct typing.
  let { value = $bindable('09:00'), onchange }: {
    value?: string;
    onchange?: (v: string) => void;
  } = $props();

  // Always parse from `value` directly — never rely on $derived inside event handlers.
  function parsed(): { h24: number; m: number } {
    const parts = value.split(':');
    return {
      h24: Math.max(0, Math.min(23, parseInt(parts[0] ?? '9', 10) || 0)),
      m:   Math.max(0, Math.min(59, parseInt(parts[1] ?? '0', 10) || 0)),
    };
  }

  let hour24 = $derived(parsed().h24);
  let minute  = $derived(parsed().m);
  let isAM    = $derived(hour24 < 12);
  let hour12  = $derived(hour24 % 12 === 0 ? 12 : hour24 % 12);

  function emit(h24: number, m: number) {
    h24 = ((h24 % 24) + 24) % 24;
    m   = ((m  % 60) + 60) % 60;
    value = `${String(h24).padStart(2,'0')}:${String(m).padStart(2,'0')}`;
    onchange?.(value);
  }

  function stepHour(delta: number)  { const p = parsed(); emit(p.h24 + delta, p.m); }
  function stepMinute(delta: number){ const p = parsed(); emit(p.h24, p.m + delta); }

  function toggleAmPm() {
    const { h24, m } = parsed();
    emit(h24 < 12 ? h24 + 12 : h24 - 12, m);
  }

  function onHourInput(e: Event) {
    const { m } = parsed();
    const raw = parseInt((e.target as HTMLInputElement).value, 10);
    if (isNaN(raw)) return;
    // Treat as 12-hour input; preserve AM/PM
    const { h24 } = parsed();
    const wasAM = h24 < 12;
    let h24new = raw % 12;
    if (!wasAM) h24new += 12;
    emit(h24new, m);
  }

  function onMinuteInput(e: Event) {
    const { h24 } = parsed();
    const raw = parseInt((e.target as HTMLInputElement).value, 10);
    if (!isNaN(raw)) emit(h24, raw);
  }
</script>

<div class="flex items-center gap-1 select-none">
  <!-- Hour -->
  <div class="flex flex-col items-center">
    <button onclick={() => stepHour(1)}  class="p-1 hover:text-indigo-600 text-gray-500 leading-none">▲</button>
    <input
      type="number" min="1" max="12"
      value={hour12}
      oninput={onHourInput}
      class="w-10 text-center text-lg font-mono font-semibold border border-gray-200 rounded
             focus:outline-none focus:ring-2 focus:ring-indigo-400 [appearance:textfield]
             [&::-webkit-outer-spin-button]:appearance-none [&::-webkit-inner-spin-button]:appearance-none"
    />
    <button onclick={() => stepHour(-1)} class="p-1 hover:text-indigo-600 text-gray-500 leading-none">▼</button>
  </div>
  <span class="text-xl font-bold text-gray-400 pb-1">:</span>
  <!-- Minute -->
  <div class="flex flex-col items-center">
    <button onclick={() => stepMinute(1)}  class="p-1 hover:text-indigo-600 text-gray-500 leading-none">▲</button>
    <input
      type="number" min="0" max="59"
      value={minute}
      oninput={onMinuteInput}
      class="w-10 text-center text-lg font-mono font-semibold border border-gray-200 rounded
             focus:outline-none focus:ring-2 focus:ring-indigo-400 [appearance:textfield]
             [&::-webkit-outer-spin-button]:appearance-none [&::-webkit-inner-spin-button]:appearance-none"
    />
    <button onclick={() => stepMinute(-1)} class="p-1 hover:text-indigo-600 text-gray-500 leading-none">▼</button>
  </div>
  <!-- AM/PM -->
  <button
    onclick={toggleAmPm}
    class="ml-1 w-12 py-1 rounded text-sm font-semibold border border-gray-300 hover:border-indigo-400
           text-gray-700 hover:text-indigo-600 transition-colors"
  >
    {isAM ? 'AM' : 'PM'}
  </button>
</div>
