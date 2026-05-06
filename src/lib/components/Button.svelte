<!-- Button.svelte -->
<script lang="ts">
    import type { Snippet } from "svelte";

    type Variant = "primary" | "danger";

    interface Props {
        onclick?: () => void;
        variant?: Variant;
        disabled?: boolean;
        class?: string;
        children: Snippet;
    }

    let {
        onclick,
        variant = "primary",
        disabled = false,
        class: className = "",
        children,
    }: Props = $props();

    const variants: Record<Variant, string> = {
        primary:
            "bg-primary text-white hover:bg-primary-light disabled:hover:bg-primary",
        danger: "bg-error text-white hover:bg-error-dark disabled:hover:bg-error",
    };
</script>

<button
    type="button"
    {onclick}
    {disabled}
    class="px-4 py-2 text-sm font-medium transition-colors
         focus:outline-none focus:ring-2 focus:ring-offset-2
         disabled:opacity-50 disabled:cursor-not-allowed
         {variants[variant]} {className}"
>
    {@render children()}
</button>
