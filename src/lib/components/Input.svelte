<!-- Input.svelte -->
<script lang="ts">
    interface Props {
        value?: string;
        name?: string;
        disabled?: boolean;
        placeholder?: string;
        type?: string;
        class?: string;
        oninput?: (event: Event) => void;
    }

    let {
        value = $bindable(""),
        name = "",
        disabled = false,
        placeholder = "",
        type = "text",
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
    class="w-full px-3 py-2 border text-sm
         bg-white
         focus:outline-none focus:ring-2 focus:ring-primary focus:border-transparent
         disabled:bg-gray disabled:cursor-not-allowed disabled:opacity-60
         {className}"
    {oninput}
/>
