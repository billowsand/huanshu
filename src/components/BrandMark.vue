<script setup lang="ts">
import { computed } from "vue";

const props = withDefaults(
    defineProps<{
        size?: number | string;
        title?: string;
    }>(),
    {
        size: 24,
        title: "幻述",
    },
);

const pixelSize = computed(() =>
    typeof props.size === "number" ? `${props.size}px` : props.size,
);
</script>

<template>
    <span
        class="brand-mark"
        :style="{ width: pixelSize, height: pixelSize }"
        :title="title"
        aria-hidden="true"
    >
        <svg viewBox="0 0 64 64" fill="none" xmlns="http://www.w3.org/2000/svg">
            <defs>
                <linearGradient
                    id="brand-cool"
                    x1="14"
                    y1="20"
                    x2="44"
                    y2="42"
                    gradientUnits="userSpaceOnUse"
                >
                    <stop stop-color="#70E1F7" />
                    <stop offset="1" stop-color="#2090C9" />
                </linearGradient>
                <linearGradient
                    id="brand-warm"
                    x1="18"
                    y1="15"
                    x2="51"
                    y2="36"
                    gradientUnits="userSpaceOnUse"
                >
                    <stop stop-color="#FFD992" />
                    <stop offset="1" stop-color="#F0A63F" />
                </linearGradient>
                <linearGradient
                    id="brand-sheet"
                    x1="16"
                    y1="16"
                    x2="48"
                    y2="42"
                    gradientUnits="userSpaceOnUse"
                >
                    <stop stop-color="#FFF7E9" />
                    <stop offset="1" stop-color="#F7E4C4" />
                </linearGradient>
            </defs>

            <rect
                x="9"
                y="18"
                width="34"
                height="23"
                rx="7"
                transform="rotate(-11 9 18)"
                fill="url(#brand-cool)"
                opacity="0.3"
            />
            <rect
                x="19"
                y="12"
                width="33"
                height="23"
                rx="7"
                transform="rotate(8 19 12)"
                fill="url(#brand-warm)"
                opacity="0.4"
            />
            <rect
                x="14"
                y="17"
                width="36"
                height="25"
                rx="8"
                transform="rotate(-3 14 17)"
                fill="url(#brand-sheet)"
            />
            <rect
                x="14"
                y="17"
                width="36"
                height="25"
                rx="8"
                transform="rotate(-3 14 17)"
                stroke="#15324E"
                stroke-opacity="0.12"
            />

            <rect
                x="24"
                y="19.5"
                width="15"
                height="15"
                rx="4"
                transform="rotate(45 24 19.5)"
                stroke="#163C63"
                stroke-width="2.6"
            />
            <circle cx="31.5" cy="27" r="3.2" fill="#F4B24B" />

            <path
                d="M45 11.5L46.8 16.2L51.5 18L46.8 19.8L45 24.5L43.2 19.8L38.5 18L43.2 16.2L45 11.5Z"
                fill="#59D1F2"
            />
        </svg>
    </span>
</template>

<style scoped>
.brand-mark {
    display: inline-flex;
    align-items: center;
    justify-content: center;
    line-height: 0;
    flex-shrink: 0;
}

.brand-mark svg {
    width: 100%;
    height: 100%;
    display: block;
}
</style>
