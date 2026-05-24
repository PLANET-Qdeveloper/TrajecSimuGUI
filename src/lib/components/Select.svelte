<script lang="ts">
  type Option = {
    value: string;
    label: string;
  };

  interface Props {
    options?: Option[];
    value?: string;
    placeholder?: string;
    disabled?: boolean;
    class?: string;
    // 開く方向を指定するプロパティを追加（デフォルトは "down"）
    direction?: "down" | "up";
    onchange?: (value: string) => void;
    onpush?: () => void;
  }

  let {
    options = [],
    value = $bindable(""),
    placeholder = "選択してください",
    disabled = false,
    class: className = "",
    direction = "down", // 追加
    onchange,
    onpush,
  }: Props = $props();

  let isOpen = $state(false);
  let containerRef: HTMLDivElement;

  let selectedOption = $derived(options.find((opt) => opt.value === value));

  function toggle() {
    if (!disabled) {
      const wasOpen = isOpen;
      isOpen = !isOpen;
      if (!wasOpen && isOpen) {
        onpush?.();
      }
    }
  }

  function select(option: Option) {
    value = option.value;
    isOpen = false;
    onchange?.(option.value);
    containerRef.querySelector("button")?.focus();
  }

  function handleKeydown(e: KeyboardEvent) {
    if (disabled) return;

    switch (e.key) {
      case "Enter":
      case " ":
        e.preventDefault();
        toggle();
        break;
      case "Escape":
        isOpen = false;
        break;
      case "ArrowDown":
        e.preventDefault();
        if (!isOpen) {
          isOpen = true;
        } else {
          const currentIndex = options.findIndex((opt) => opt.value === value);
          const nextIndex = Math.min(currentIndex + 1, options.length - 1);
          value = options[nextIndex].value;
        }
        break;
      case "ArrowUp":
        e.preventDefault();
        if (isOpen) {
          const currentIndex = options.findIndex((opt) => opt.value === value);
          const prevIndex = Math.max(currentIndex - 1, 0);
          value = options[prevIndex].value;
        }
        break;
    }
  }

  function handleClickOutside(e: MouseEvent) {
    if (containerRef && !containerRef.contains(e.target as Node)) {
      isOpen = false;
    }
  }
</script>

<svelte:window on:click={handleClickOutside} />

<div bind:this={containerRef} class="relative w-full {className}">
  <button
    type="button"
    onclick={toggle}
    onkeydown={handleKeydown}
    class="
            w-full px-2 py-1 text-left
            border
            text-sm
            cursor-pointer
            bg-white
            disabled:bg-gray disabled:cursor-not-allowed disabled:opacity-60
            flex items-center justify-between
            transition-shadow duration-200
            {isOpen
      ? 'border-primary ring-2 ring-primary outline-none'
      : 'hover:border-primary focus:outline-none focus:border-primary focus:ring-1 focus:ring-primary'}
            {className}
        "
    {disabled}
    aria-haspopup="listbox"
    aria-expanded={isOpen}
  >
    <span class="text-black">
      {selectedOption?.label ?? placeholder}
    </span>

    <svg
      class="w-5 h-5 text-black transition-transform duration-150"
      class:rotate-180={isOpen}
      fill="none"
      stroke="currentColor"
      viewBox="0 0 24 24"
    >
      {#if direction === "up"}
        <path
          stroke-linecap="round"
          stroke-linejoin="round"
          stroke-width="2"
          d="M19 15l-7-7-7 7"
        />
      {:else}
        <path
          stroke-linecap="round"
          stroke-linejoin="round"
          stroke-width="2"
          d="M19 9l-7 7-7-7"
        />
      {/if}
    </svg>
  </button>

  {#if isOpen}
    <ul
      class="
                absolute z-50 w-full
                border border-primary bg-white
                max-h-60 overflow-auto
                py-1
                {direction === 'up' ? 'bottom-full mb-1' : 'top-full mt-1'}
            "
      role="listbox"
    >
      {#each options as option (option.value)}
        <li
          role="option"
          aria-selected={option.value === value}
          onclick={() => select(option)}
          onkeydown={(e) => e.key === "Enter" && select(option)}
          class="
                        px-4 py-2.5 cursor-pointer
                        transition-colors duration-150
                        hover:bg-primary hover:text-white
                        {option.value === value
            ? 'bg-primary text-white font-medium'
            : 'text-black'}
                    "
        >
          <div class="flex items-center justify-between">
            <span>{option.label}</span>
            {#if option.value === value}
              <svg
                class="w-5 h-5 text-white"
                fill="currentColor"
                viewBox="0 0 20 20"
              >
                <path
                  fill-rule="evenodd"
                  d="M16.707 5.293a1 1 0 010 1.414l-8 8a1 1 0 01-1.414 0l-4-4a1 1 0 011.414-1.414L8 12.586l7.293-7.293a1 1 0 011.414 0z"
                  clip-rule="evenodd"
                />
              </svg>
            {/if}
          </div>
        </li>
      {/each}

      {#if options.length === 0}
        <li class="px-4 py-2.5 text-gray text-center">
          オプションがありません
        </li>
      {/if}
    </ul>
  {/if}
</div>
