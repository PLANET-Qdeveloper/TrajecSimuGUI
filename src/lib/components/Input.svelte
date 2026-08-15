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
    /** WebKit/Safari (and Tauri's WKWebView) auto-capitalizes the first
     * letter by default; pass "off" to disable it (e.g. for raw command
     * input where casing matters). */
    autocapitalize?: "off" | "none" | "sentences" | "words" | "characters";
    autocorrect?: "off" | "on";
    spellcheck?: boolean;
    oninput?: (event: Event) => void;
    onkeydown?: (event: KeyboardEvent) => void;
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
    autocapitalize,
    autocorrect,
    spellcheck,
    oninput,
    onkeydown,
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
  {autocapitalize}
  {autocorrect}
  {spellcheck}
  class="w-full px-2 py-0.5 border text-xs
         bg-white
         focus:outline-none focus:ring-1 focus:ring-primary focus:border-transparent
         disabled:bg-gray disabled:cursor-not-allowed disabled:opacity-60
         {className}"
  {oninput}
  {onkeydown}
/>
