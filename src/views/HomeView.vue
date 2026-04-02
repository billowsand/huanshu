<script setup lang="ts">
import { onMounted, ref } from "vue";
import { useRouter } from "vue-router";
import { invoke } from "@tauri-apps/api/core";
import { save } from "@tauri-apps/plugin-dialog";
import { writeFile } from "@tauri-apps/plugin-fs";
import { useProjectsStore } from "../stores/projects";
import SkeletonCard from "../components/SkeletonCard.vue";
import ThemeToggle from "../components/ThemeToggle.vue";
import BrandMark from "../components/BrandMark.vue";

const router = useRouter();
const projects = useProjectsStore();
const confirmDelete = ref<number | null>(null);
const pendingDeleteId = ref<number | null>(null);
const exportingId = ref<number | null>(null);
const importing = ref(false);
const showThemePicker = ref(false);

onMounted(async () => {
    projects.refresh();
    invoke("ensure_icon_embeddings").catch(() => {});
});

function handleDelete(id: number) {
    pendingDeleteId.value = id;
}

async function confirmDeleteProject() {
    if (pendingDeleteId.value !== null) {
        await projects.remove(pendingDeleteId.value);
        pendingDeleteId.value = null;
    }
}

function cancelDelete() {
    pendingDeleteId.value = null;
}

async function handleExport(id: number) {
    const p = projects.list.find((proj) => proj.id === id);
    if (!p) return;
    exportingId.value = id;
    try {
        const zipData = await invoke<number[]>("export_project", {
            projectId: id,
        });
        const bytes = new Uint8Array(zipData);
        const filename = `${p.name}.keynn`;
        const path = await save({
            defaultPath: filename,
            filters: [{ name: "Keynn Archive", extensions: ["keynn"] }],
        });
        if (path) {
            await writeFile(path, bytes);
        }
    } catch (e) {
        console.error("Export failed:", e);
    } finally {
        exportingId.value = null;
    }
}

async function handleImport() {
    const { open } = await import("@tauri-apps/plugin-dialog");
    const selected = await open({
        filters: [{ name: "Keynn Archive", extensions: ["keynn"] }],
    });
    if (!selected) return;
    importing.value = true;
    try {
        const { readFile } = await import("@tauri-apps/plugin-fs");
        const bytes = await readFile(selected as string);
        const uint8 = new Uint8Array(bytes);
        await invoke<number>("import_project", { zipData: Array.from(uint8) });
        await projects.refresh();
    } catch (e) {
        console.error("Import failed:", e);
    } finally {
        importing.value = false;
    }
}
</script>

<template>
    <div class="home-layout" @click="showThemePicker = false">
        <!-- ── 操作栏（不重复 TitleBar 的品牌信息）─────────────────── -->
        <header class="top-bar">
            <div class="bar-left">
                <span class="page-crumb">项目</span>
            </div>
            <div class="top-actions">
                <!-- 主题选择器：图标触发，浮层展开 -->
                <div class="theme-anchor" @click.stop>
                    <button
                        class="btn btn-ghost icon-btn"
                        :class="{ active: showThemePicker }"
                        title="切换主题"
                        @click="showThemePicker = !showThemePicker"
                    >
                        <span class="i-carbon:color-palette" />
                    </button>
                    <Transition name="pop">
                        <div v-if="showThemePicker" class="theme-popover">
                            <ThemeToggle />
                        </div>
                    </Transition>
                </div>

                <div class="bar-divider" />

                <button
                    class="btn btn-ghost"
                    :disabled="importing"
                    @click="handleImport"
                >
                    <span v-if="importing" class="i-carbon:renew spin" />
                    <span v-else class="i-carbon:folder-up" />
                    {{ importing ? "导入中…" : "导入" }}
                </button>
                <button
                    class="btn btn-ghost icon-btn"
                    title="设置"
                    @click="router.push('/settings')"
                >
                    <span class="i-carbon:settings" />
                </button>
                <button class="btn btn-primary" @click="router.push('/new')">
                    <span class="i-carbon:add" /> 新建演示
                </button>
            </div>
        </header>

        <div class="home-body">
            <!-- ── 页面标题区 ──────────────────────────────────────── -->
            <div class="page-heading">
                <div class="heading-brand">
                    <div class="heading-mark">
                        <BrandMark :size="28" />
                    </div>
                    <div class="heading-copy">
                        <span class="heading-eyebrow">幻述 Studio</span>
                        <h1 class="page-title">我的演示文稿</h1>
                    </div>
                </div>
                <span
                    v-if="!projects.loading && projects.list.length > 0"
                    class="project-count"
                >
                    {{ projects.list.length }} 个项目
                </span>
            </div>

            <!-- ── 加载骨架 ──────────────────────────────────────── -->
            <div v-if="projects.loading" class="project-grid">
                <div v-for="i in 4" :key="i" class="project-card loading-card">
                    <div class="card-icon-wrap">
                        <BrandMark :size="24" class="card-icon" />
                    </div>
                    <div class="card-body">
                        <SkeletonCard :rows="2" height="13px" />
                    </div>
                </div>
            </div>

            <!-- ── 空状态 ─────────────────────────────────────────── -->
            <div v-else-if="projects.list.length === 0" class="empty-state">
                <div class="empty-illustration">
                    <div class="slide-stack">
                        <div class="slide-card-bg bg-1" />
                        <div class="slide-card-bg bg-2" />
                        <div class="slide-card-bg bg-3" />
                    </div>
                    <div class="empty-icon-circle">
                        <BrandMark :size="30" />
                    </div>
                </div>
                <p class="empty-title">还没有任何演示文稿</p>
                <p class="empty-desc">
                    上传 Markdown 文件，AI 自动生成漂亮的演示文稿
                </p>
                <button class="btn btn-primary mt" @click="router.push('/new')">
                    <span class="i-carbon:add" /> 新建演示
                </button>
            </div>

            <!-- ── 项目列表 ──────────────────────────────────────── -->
            <div v-else class="project-grid">
                <div
                    v-for="(p, idx) in projects.list"
                    :key="p.id"
                    class="project-card"
                    :style="{ animationDelay: `${idx * 50}ms` }"
                    @click="router.push(`/project/${p.id}`)"
                >
                    <div class="card-accent" />
                    <div class="card-icon-wrap">
                        <BrandMark :size="24" class="card-icon" />
                    </div>
                    <div class="card-body">
                        <div class="card-name">{{ p.name }}</div>
                        <div class="card-meta">
                            <span class="badge"
                                >{{ p.slide_count }} 张幻灯片</span
                            >
                            <span class="card-date">{{
                                projects.formatDate(p.updated_at)
                            }}</span>
                        </div>
                    </div>
                    <div class="card-actions" @click.stop>
                        <button
                            class="btn btn-ghost icon-btn"
                            :class="{ loading: exportingId === p.id }"
                            :title="
                                exportingId === p.id
                                    ? '导出中…'
                                    : '导出为 .keynn'
                            "
                            :disabled="exportingId === p.id"
                            @click="handleExport(p.id)"
                        >
                            <span
                                v-if="exportingId === p.id"
                                class="i-carbon:renew spin"
                            />
                            <span v-else class="i-carbon:download" />
                        </button>
                        <button
                            class="btn btn-ghost icon-btn"
                            title="删除"
                            @click="handleDelete(p.id)"
                        >
                            <span class="i-carbon:trash-can" />
                        </button>
                    </div>
                </div>
            </div>
        </div>

        <!-- 删除确认对话框 -->
        <Teleport to="body">
            <Transition name="modal">
                <div v-if="pendingDeleteId !== null" class="modal-overlay" @click.self="cancelDelete">
                    <div class="modal-box">
                        <div class="modal-icon">
                            <span class="i-carbon:warning" />
                        </div>
                        <h3 class="modal-title">确认删除</h3>
                        <p class="modal-desc">删除后无法恢复，确定要删除这个项目吗？</p>
                        <div class="modal-actions">
                            <button class="btn btn-ghost" @click="cancelDelete">取消</button>
                            <button class="btn btn-danger" @click="confirmDeleteProject">删除</button>
                        </div>
                    </div>
                </div>
            </Transition>
        </Teleport>
    </div>
</template>

<style scoped>
.home-layout {
    display: flex;
    flex-direction: column;
    height: 100%;
    background: var(--studio-bg);
}

/* ── 操作栏（不含品牌 Logo）─────────────────────────────── */
.top-bar {
    height: 48px;
    display: flex;
    align-items: center;
    justify-content: space-between;
    padding: 0 1.25rem;
    border-bottom: 1px solid var(--studio-border);
    background: var(--studio-panel-2);
    flex-shrink: 0;
}

.bar-left {
    display: flex;
    align-items: center;
}

.page-crumb {
    font-size: 0.75rem;
    font-weight: 600;
    color: var(--studio-muted);
    letter-spacing: 0.04em;
    text-transform: uppercase;
}

.top-actions {
    display: flex;
    gap: 0.375rem;
    align-items: center;
}

.bar-divider {
    width: 1px;
    height: 18px;
    background: var(--studio-border);
    margin: 0 0.25rem;
    flex-shrink: 0;
}

/* ── 主题弹出层 ────────────────────────────────────────── */
.theme-anchor {
    position: relative;
}

.theme-popover {
    position: absolute;
    top: calc(100% + 8px);
    right: 0;
    z-index: 200;
    background: var(--studio-panel);
    border: 1px solid var(--studio-border);
    border-radius: 10px;
    box-shadow: 0 8px 32px rgba(0, 0, 0, 0.3);
    overflow: hidden;
    min-width: 200px;
}

/* 弹出动画 */
.pop-enter-active,
.pop-leave-active {
    transition:
        opacity 0.15s ease,
        transform 0.15s ease;
}
.pop-enter-from,
.pop-leave-to {
    opacity: 0;
    transform: translateY(-6px) scale(0.97);
}

/* ── 页面标题区 ─────────────────────────────────────────── */
.page-heading {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 0.75rem;
    margin-bottom: 1.25rem;
}

.heading-brand {
    display: flex;
    align-items: center;
    gap: 0.9rem;
}

.heading-mark {
    width: 46px;
    height: 46px;
    border-radius: 14px;
    display: flex;
    align-items: center;
    justify-content: center;
    background:
        radial-gradient(
            circle at 30% 25%,
            rgba(255, 213, 140, 0.18),
            transparent 55%
        ),
        linear-gradient(145deg, rgba(23, 34, 51, 0.96), rgba(10, 18, 29, 0.96));
    border: 1px solid rgba(110, 160, 210, 0.14);
    box-shadow: 0 12px 30px rgba(0, 0, 0, 0.18);
}

.heading-copy {
    display: flex;
    flex-direction: column;
    gap: 0.15rem;
}

.heading-eyebrow {
    font-size: 0.72rem;
    font-weight: 700;
    letter-spacing: 0.1em;
    text-transform: uppercase;
    color: var(--studio-primary);
}

.page-title {
    font-size: 1.125rem;
    font-weight: 700;
    color: var(--studio-text);
    margin: 0;
    letter-spacing: -0.02em;
}

.project-count {
    font-size: 0.75rem;
    color: var(--studio-muted);
    font-weight: 500;
    white-space: nowrap;
}

/* ── 内容区 ─────────────────────────────────────────────── */
.home-body {
    flex: 1;
    overflow-y: auto;
    padding: 1.5rem;
    max-width: 860px;
    margin: 0 auto;
    width: 100%;
    box-sizing: border-box;
}

/* ── 加载骨架 ────────────────────────────────────────────── */
.loading-card {
    pointer-events: none;
    opacity: 0.6;
}

.card-icon-wrap {
    width: 40px;
    height: 40px;
    border-radius: 8px;
    background: var(--studio-surface);
    display: flex;
    align-items: center;
    justify-content: center;
    flex-shrink: 0;
}

.card-icon {
    font-size: 1.25rem;
    color: var(--studio-muted);
}

/* ── 空状态 ─────────────────────────────────────────────── */
.empty-state {
    display: flex;
    flex-direction: column;
    align-items: center;
    justify-content: center;
    gap: 0.875rem;
    height: 65vh;
    color: var(--studio-muted);
    text-align: center;
}

.empty-illustration {
    position: relative;
    width: 120px;
    height: 90px;
    margin-bottom: 0.5rem;
}

.slide-stack {
    position: absolute;
    inset: 0;
}

.slide-card-bg {
    position: absolute;
    width: 80px;
    height: 56px;
    border-radius: 8px;
    border: 1px solid var(--studio-border);
}

.bg-1 {
    background: var(--studio-panel);
    transform: rotate(-12deg) translateY(4px) translateX(8px);
    opacity: 0.5;
}
.bg-2 {
    background: var(--studio-panel-2);
    transform: rotate(-4deg) translateY(2px) translateX(4px);
    opacity: 0.7;
}
.bg-3 {
    background: var(--studio-surface);
    transform: rotate(4deg);
    opacity: 0.9;
}

.empty-icon-circle {
    position: absolute;
    top: 50%;
    left: 50%;
    transform: translate(-50%, -50%);
    width: 52px;
    height: 52px;
    border-radius: 50%;
    background: var(--studio-primary-bg);
    border: 1px solid var(--studio-primary-border);
    display: flex;
    align-items: center;
    justify-content: center;
    box-shadow: 0 12px 28px rgba(0, 0, 0, 0.18);
}

.empty-title {
    font-size: 1rem;
    font-weight: 600;
    color: var(--studio-text);
    margin: 0;
}

.empty-desc {
    font-size: 0.8125rem;
    color: var(--studio-muted);
    margin: 0;
}

.mt {
    margin-top: 0.5rem;
}

/* ── 项目网格 ────────────────────────────────────────────── */
.project-grid {
    display: flex;
    flex-direction: column;
    gap: 0.5rem;
}

.project-card {
    position: relative;
    display: flex;
    align-items: center;
    gap: 0.875rem;
    padding: 0.75rem 1rem 0.75rem 1.25rem;
    background: var(--studio-panel);
    border: 1px solid var(--studio-border);
    border-radius: 10px;
    cursor: pointer;
    transition: all 0.2s ease;
    overflow: hidden;
    animation: card-in 0.3s ease backwards;
}

@keyframes card-in {
    from {
        opacity: 0;
        transform: translateY(8px);
    }
    to {
        opacity: 1;
        transform: translateY(0);
    }
}

.project-card:hover {
    border-color: var(--studio-primary-border);
    background: var(--studio-panel-2);
    transform: translateY(-2px);
    box-shadow: 0 8px 24px rgba(0, 0, 0, 0.25);
}

.card-accent {
    position: absolute;
    left: 0;
    top: 0;
    bottom: 0;
    width: 3px;
    background: var(--studio-primary);
    transform: scaleY(0);
    transition: transform 0.2s ease;
    border-radius: 3px 0 0 3px;
}

.project-card:hover .card-accent {
    transform: scaleY(1);
}

.card-icon-wrap {
    width: 40px;
    height: 40px;
    border-radius: 8px;
    background: var(--studio-primary-bg);
    border: 1px solid var(--studio-primary-border);
    display: flex;
    align-items: center;
    justify-content: center;
    flex-shrink: 0;
    transition: transform 0.2s ease;
}

.project-card:hover .card-icon-wrap {
    transform: scale(1.05);
}

.card-icon {
    color: var(--studio-primary);
}

.card-body {
    flex: 1;
    min-width: 0;
}

.card-name {
    font-size: 0.9rem;
    font-weight: 600;
    color: var(--studio-text);
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
}

.card-meta {
    display: flex;
    align-items: center;
    gap: 0.5rem;
    margin-top: 0.25rem;
}

.badge {
    font-size: 0.65rem;
    background: var(--studio-primary-bg);
    border: 1px solid var(--studio-primary-border);
    color: var(--studio-primary);
    border-radius: 10px;
    padding: 0.05rem 0.45rem;
    font-weight: 600;
}

.card-date {
    font-size: 0.7rem;
    color: var(--studio-muted);
}

.card-actions {
    flex-shrink: 0;
}

.icon-btn {
    padding: 0.35rem;
    font-size: 0.9rem;
    border-radius: 6px;
}

.icon-btn.active {
    background: var(--studio-primary-bg);
    color: var(--studio-primary);
    border-color: var(--studio-primary-border);
}

.icon-btn.danger {
    color: var(--studio-error);
    background: var(--studio-error-bg);
    border-color: var(--studio-error-border);
}

.icon-btn.loading {
    opacity: 0.6;
    cursor: wait;
}

.spin {
    animation: spin 0.7s linear infinite;
}
@keyframes spin {
    to {
        transform: rotate(360deg);
    }
}

/* ── 删除确认对话框 ─────────────────────────────────────── */
.modal-overlay {
    position: fixed;
    inset: 0;
    z-index: 1000;
    background: rgba(0, 0, 0, 0.6);
    backdrop-filter: blur(4px);
    display: flex;
    align-items: center;
    justify-content: center;
}

.modal-box {
    background: var(--studio-panel);
    border: 1px solid var(--studio-border);
    border-radius: 14px;
    padding: 1.5rem;
    width: 320px;
    text-align: center;
    box-shadow: 0 20px 60px rgba(0, 0, 0, 0.4);
}

.modal-icon {
    font-size: 2.5rem;
    color: var(--studio-error);
    margin-bottom: 0.75rem;
}

.modal-title {
    font-size: 1.1rem;
    font-weight: 700;
    color: var(--studio-text);
    margin: 0 0 0.5rem;
}

.modal-desc {
    font-size: 0.85rem;
    color: var(--studio-muted);
    margin: 0 0 1.25rem;
    line-height: 1.5;
}

.modal-actions {
    display: flex;
    gap: 0.75rem;
    justify-content: center;
}

.btn-danger {
    background: var(--studio-error-bg);
    color: var(--studio-error);
    border: 1px solid var(--studio-error-border);
}

.btn-danger:hover {
    background: var(--studio-error);
    color: #fff;
}

/* 弹窗动画 */
.modal-enter-active,
.modal-leave-active {
    transition: opacity 0.2s ease;
}
.modal-enter-active .modal-box,
.modal-leave-active .modal-box {
    transition: transform 0.2s ease, opacity 0.2s ease;
}
.modal-enter-from,
.modal-leave-to {
    opacity: 0;
}
.modal-enter-from .modal-box,
.modal-leave-to .modal-box {
    transform: scale(0.92);
    opacity: 0;
}
</style>
