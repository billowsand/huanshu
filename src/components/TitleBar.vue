<script setup lang="ts">
import { ref, onMounted, onUnmounted } from "vue";
import { getCurrentWindow } from "@tauri-apps/api/window";
import BrandMark from "./BrandMark.vue";

const appWindow = getCurrentWindow();
const isMaximized = ref(false);

async function syncMaximized() {
    isMaximized.value = await appWindow.isMaximized();
}

async function minimize() {
    await appWindow.minimize();
}

async function toggleMaximize() {
    await appWindow.toggleMaximize();
    isMaximized.value = await appWindow.isMaximized();
}

async function close() {
    await appWindow.close();
}

let unlisten: (() => void) | null = null;
onMounted(async () => {
    await syncMaximized();
    unlisten = await appWindow.onResized(syncMaximized);
});
onUnmounted(() => {
    unlisten?.();
});
</script>

<template>
    <div class="titlebar" data-tauri-drag-region>
        <div class="titlebar-drag-zone" data-tauri-drag-region>
            <span class="titlebar-brand-chip" data-tauri-drag-region>
                <BrandMark :size="16" />
            </span>
            <span class="titlebar-title" data-tauri-drag-region>幻述</span>
        </div>
        <div class="titlebar-controls">
            <button
                class="ctrl-btn ctrl-minimize"
                title="最小化"
                @click="minimize"
            >
                <svg width="10" height="1" viewBox="0 0 10 1">
                    <rect width="10" height="1" fill="currentColor" />
                </svg>
            </button>
            <button
                class="ctrl-btn ctrl-maximize"
                :title="isMaximized ? '还原' : '最大化'"
                @click="toggleMaximize"
            >
                <svg
                    v-if="!isMaximized"
                    width="10"
                    height="10"
                    viewBox="0 0 10 10"
                >
                    <rect
                        x="0.5"
                        y="0.5"
                        width="9"
                        height="9"
                        fill="none"
                        stroke="currentColor"
                        stroke-width="1"
                    />
                </svg>
                <svg v-else width="10" height="10" viewBox="0 0 10 10">
                    <rect
                        x="2"
                        y="0"
                        width="8"
                        height="8"
                        fill="none"
                        stroke="currentColor"
                        stroke-width="1"
                    />
                    <rect
                        x="0"
                        y="2"
                        width="8"
                        height="8"
                        fill="var(--studio-panel)"
                        stroke="currentColor"
                        stroke-width="1"
                    />
                </svg>
            </button>
            <button class="ctrl-btn ctrl-close" title="关闭" @click="close">
                <svg width="10" height="10" viewBox="0 0 10 10">
                    <line
                        x1="0"
                        y1="0"
                        x2="10"
                        y2="10"
                        stroke="currentColor"
                        stroke-width="1.2"
                    />
                    <line
                        x1="10"
                        y1="0"
                        x2="0"
                        y2="10"
                        stroke="currentColor"
                        stroke-width="1.2"
                    />
                </svg>
            </button>
        </div>
    </div>
</template>

<style scoped>
.titlebar {
    position: fixed;
    top: 0;
    left: 0;
    right: 0;
    height: var(--titlebar-height);
    z-index: 10000;
    display: flex;
    align-items: center;
    justify-content: space-between;
    background: var(--studio-panel);
    border-bottom: 1px solid var(--studio-border);
    user-select: none;
    -webkit-user-select: none;
}

.titlebar-drag-zone {
    flex: 1;
    height: 100%;
    display: flex;
    align-items: center;
    gap: 0.4rem;
    padding: 0 0.75rem;
    cursor: default;
}

.titlebar-brand-chip {
    width: 22px;
    height: 22px;
    border-radius: 7px;
    display: inline-flex;
    align-items: center;
    justify-content: center;
    background:
        radial-gradient(
            circle at 30% 25%,
            rgba(255, 220, 160, 0.18),
            transparent 55%
        ),
        linear-gradient(145deg, rgba(26, 38, 57, 0.96), rgba(12, 19, 31, 0.96));
    border: 1px solid rgba(120, 170, 220, 0.16);
    box-shadow: inset 0 1px 0 rgba(255, 255, 255, 0.05);
}

.titlebar-title {
    font-size: 0.77rem;
    font-weight: 700;
    color: var(--studio-text);
    letter-spacing: 0.04em;
}

.titlebar-controls {
    display: flex;
    align-items: stretch;
    height: 100%;
}

.ctrl-btn {
    width: 46px;
    height: 100%;
    border: none;
    background: transparent;
    color: var(--studio-muted);
    display: flex;
    align-items: center;
    justify-content: center;
    cursor: pointer;
    transition:
        background 0.15s,
        color 0.15s;
    outline: none;
    padding: 0;
}

.ctrl-btn:hover {
    background: var(--studio-surface);
    color: var(--studio-text);
}

.ctrl-close:hover {
    background: #e81123;
    color: #ffffff;
}
</style>
