<!-- Input.svelte -->
<script lang="ts">
  interface Props {
    value?: string | number;
    name?: string;
    disabled?: boolean;
    placeholder?: string;
    type?: string;
    step?: number | string;
    min?: number | string;
    max?: number | string;
    class?: string;
    oninput?: (event: Event) => void;
  }

  let {
    value = $bindable(""),
    name = "",
    disabled = false,
    placeholder = "",
    type = "text",
    step,
    min,
    max,
    class: className = "",
    oninput,
  }: Props = $props();
</script>

<input
  {type}
  bind:value
  {name}
  {disabled}
  {placeholder}
  {step}
  {min}
  {max}
  class="w-full px-2 py-0.5 border text-xs
         bg-white
         focus:outline-none focus:ring-1 focus:ring-primary focus:border-transparent
         disabled:bg-gray disabled:cursor-not-allowed disabled:opacity-60
         {className}"
  {oninput}
/>
