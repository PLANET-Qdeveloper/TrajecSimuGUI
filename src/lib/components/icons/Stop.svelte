<script lang="ts">
    interface Props {
        size?: number | string;
        color?: string;
        strokeWidth?: number | string;
        class?: string;
        [key: string]: unknown;
    }

    let {
        size = 24,
        color = "currentColor",
        strokeWidth = 2,
        class: className = "",
        ...rest
    }: Props = $props();
</script>

<svg
    xmlns="http://www.w3.org/2000/svg"
    width={size}
    height={size}
    viewBox="0 0 24 24"
    fill="none"
    stroke={color}
    stroke-width={strokeWidth}
    stroke-linecap="round"
    stroke-linejoin="round"
    class="play-icon {className}"
    {...rest}
>
    <rect x="3" y="3" width="18" height="18" rx="2" ry="2"></rect>
</svg>
