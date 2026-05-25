<script lang="ts">
  interface Props {
    value?: number;
    step?: number;
    min?: number;
    max?: number;
    disabled?: boolean;
    class?: string;
    onValueChange?: (value: number) => void;
  }

  let {
    value = $bindable<number | undefined>(undefined),
    step = 1,
    min,
    max,
    disabled = false,
    class: className = "",
    onValueChange,
  }: Props = $props();

  function precision(n: number): number {
    const s = n.toString();
    const dot = s.indexOf(".");
    return dot === -1 ? 0 : s.length - dot - 1;
  }

  function increment() {
    const next = parseFloat(((value ?? 0) + step).toFixed(precision(step)));
    if (max !== undefined && next > max) return;
    value = next;
    onValueChange?.(next);
  }

  function decrement() {
    const next = parseFloat(((value ?? 0) - step).toFixed(precision(step)));
    if (min !== undefined && next < min) return;
    value = next;
    onValueChange?.(next);
  }

  function handleInput(e: Event) {
    const v = parseFloat((e.target as HTMLInputElement).value);
    if (!isNaN(v)) onValueChange?.(v);
  }
</script>

<div class="flex {className}">
  <input
    type="number"
    bind:value
    {step}
    {min}
    {max}
    {disabled}
    oninput={handleInput}
    class="flex-1 min-w-0 px-2 py-0.5 border-y border-l text-xs bg-white
               focus:outline-none focus:ring-1 focus:ring-primary
               disabled:bg-gray disabled:cursor-not-allowed disabled:opacity-60
               spin-none"
  />
  <div class="flex flex-col">
    <button
      type="button"
      onclick={increment}
      {disabled}
      tabindex="-1"
      class="flex-1 px-1.5 border text-[9px] leading-none bg-white
                   hover:bg-gray-50 active:bg-gray-100
                   disabled:opacity-50 disabled:cursor-not-allowed select-none"
      >▲</button
    >
    <button
      type="button"
      onclick={decrement}
      {disabled}
      tabindex="-1"
      class="flex-1 px-1.5 border border-t-0 text-[9px] leading-none bg-white
                   hover:bg-gray-50 active:bg-gray-100
                   disabled:opacity-50 disabled:cursor-not-allowed select-none"
      >▼</button
    >
  </div>
</div>

<style>
  .spin-none::-webkit-inner-spin-button,
  .spin-none::-webkit-outer-spin-button {
    appearance: none;
    margin: 0;
  }

  .spin-none {
    appearance: textfield;
  }
</style>
