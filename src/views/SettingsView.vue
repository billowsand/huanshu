<script setup lang="ts">
import { computed, ref, onMounted } from "vue";
import { useRouter } from "vue-router";
import { useConfigStore, type ModelTarget } from "../stores/config";
import { useAppSettingsStore } from "../stores/appSettings";
import { invoke } from "@tauri-apps/api/core";
import BrandMark from "../components/BrandMark.vue";

const router = useRouter();
const config = useConfigStore();
const appSettings = useAppSettingsStore();

const modelLists = ref<Record<ModelTarget, string[]>>({
    llm: [],
    embedding: [],
    multimodal: [],
});
const loadingModels = ref<Record<ModelTarget, boolean>>({
    llm: false,
    embedding: false,
    multimodal: false,
});
const modelErrors = ref<Record<ModelTarget, string>>({
    llm: "",
    embedding: "",
    multimodal: "",
});
const saved = ref(false);
const initializingEmbeddings = ref(false);
const embeddingStatus = ref("");

const embeddingModelCurrent = computed(
    () => config.settings.embedding.model.trim(),
);
const embeddingModelInitialized = computed(
    () => appSettings.settings.initialized_embedding_model.trim(),
);
const embeddingCacheReady = computed(
    () =>
        appSettings.settings.embeddings_ready &&
        embeddingModelCurrent.value.length > 0 &&
        embeddingModelCurrent.value === embeddingModelInitialized.value,
);
const embeddingStatusLabel = computed(() => {
    if (embeddingCacheReady.value) {
        return "已初始化";
    }
    if (appSettings.settings.embeddings_ready && embeddingModelInitialized.value) {
        return "模型已变更，需重新初始化";
    }
    return "未初始化";
});

const modelSections: Array<{
    key: ModelTarget;
    title: string;
    desc: string;
    hint: string;
}> = [
    {
        key: "llm",
        title: "大语言模型",
        desc: "负责 Markdown 优化、页面规划、布局选择和幻灯片正文生成。",
        hint: "推荐 qwen/qwen3.5-9b 或更强的指令模型",
    },
    {
        key: "embedding",
        title: "向量模型",
        desc: "负责图标语义匹配与向量检索，需支持 /v1/embeddings。",
        hint: "推荐 text-embedding-bge-m3 或同类 embedding 模型",
    },
    {
        key: "multimodal",
        title: "多模态模型",
        desc: "负责图片理解与自动描述，不再复用文本生成模型配置。",
        hint: "推荐支持图像输入的视觉模型，如 qwen2.5-vl 系列",
    },
];

onMounted(async () => {
    await config.load();
    await appSettings.load();
});

async function fetchModels(target: ModelTarget) {
    loadingModels.value[target] = true;
    modelErrors.value[target] = "";
    try {
        modelLists.value[target] = await config.listModels(
            target,
            { ...config.settings[target] },
        );
    } catch (e: unknown) {
        modelErrors.value[target] = String(e);
    } finally {
        loadingModels.value[target] = false;
    }
}

async function save() {
    await config.save();
    saved.value = true;
    setTimeout(() => {
        saved.value = false;
    }, 2000);
}

async function initEmbeddings() {
    initializingEmbeddings.value = true;
    embeddingStatus.value = "正在初始化...";
    try {
        await invoke("ensure_icon_embeddings");
        await appSettings.load();
        embeddingStatus.value = "初始化完成！";
    } catch (e) {
        embeddingStatus.value = `失败: ${e}`;
    } finally {
        initializingEmbeddings.value = false;
    }
}
</script>

<template>
    <div class="settings-layout">
        <header class="top-bar">
            <button class="btn btn-ghost" @click="router.push('/')">
                <span class="i-carbon:arrow-left" /> 返回 Studio
            </button>
            <div class="page-title">
                <span class="settings-brand-mark">
                    <BrandMark :size="18" />
                </span>
                <span>幻述设置</span>
            </div>
            <button
                class="btn btn-primary"
                :disabled="config.saving"
                @click="save"
            >
                <span v-if="saved" class="i-carbon:checkmark" />
                <span v-else class="i-carbon:save" />
                {{ saved ? "已保存" : "保存设置" }}
            </button>
        </header>

        <div class="settings-body">
            <div class="settings-card">
                <h2 class="section-title">模型服务配置</h2>
                <p class="section-desc">
                    现在支持分别配置大语言模型、向量模型和多模态模型。三类能力可指向不同
                    URL、API Key 和模型 ID。
                </p>

                <div class="model-service-grid">
                    <section
                        v-for="section in modelSections"
                        :key="section.key"
                        class="service-panel"
                    >
                        <div class="model-header">
                            <div>
                                <h3>{{ section.title }}</h3>
                                <p class="service-desc">{{ section.desc }}</p>
                            </div>
                            <button
                                class="btn"
                                :disabled="loadingModels[section.key]"
                                @click="fetchModels(section.key)"
                            >
                                <span
                                    v-if="loadingModels[section.key]"
                                    class="i-carbon:renew spin"
                                />
                                <span v-else class="i-carbon:refresh" />
                                {{
                                    loadingModels[section.key]
                                        ? "加载中..."
                                        : "获取模型"
                                }}
                            </button>
                        </div>

                        <p v-if="modelErrors[section.key]" class="error-msg">
                            {{ modelErrors[section.key] }}
                        </p>

                        <div class="field-group">
                            <div class="field">
                                <label>API 地址</label>
                                <input
                                    v-model="
                                        config.settings[section.key].base_url
                                    "
                                    placeholder="http://127.0.0.1:1234"
                                />
                                <p class="hint">
                                    本地常见地址：LM Studio
                                    `http://127.0.0.1:1234`，Ollama
                                    `http://127.0.0.1:11434`
                                </p>
                            </div>

                            <div class="field">
                                <label>
                                    API Key
                                    <span class="subtle"
                                        >（本地服务留空）</span
                                    >
                                </label>
                                <input
                                    v-model="config.settings[section.key].api_key"
                                    type="password"
                                    placeholder="sk-..."
                                    autocomplete="off"
                                />
                            </div>

                            <div class="field">
                                <label>模型</label>
                                <div class="select-row">
                                    <select
                                        v-model="
                                            config.settings[section.key].model
                                        "
                                    >
                                        <option
                                            v-for="m in modelLists[section.key]"
                                            :key="m"
                                            :value="m"
                                        >
                                            {{ m }}
                                        </option>
                                        <option
                                            :value="
                                                config.settings[section.key]
                                                    .model
                                            "
                                        >
                                            {{
                                                config.settings[section.key]
                                                    .model
                                            }}
                                            (当前)
                                        </option>
                                    </select>
                                    <input
                                        v-model="
                                            config.settings[section.key].model
                                        "
                                        placeholder="或手动输入模型 ID"
                                    />
                                </div>
                                <p class="hint">{{ section.hint }}</p>
                            </div>
                        </div>
                    </section>
                </div>
            </div>

            <div class="settings-card">
                <h2 class="section-title">生成参数</h2>

                <div class="field-group two-col">
                    <div class="field">
                        <label>修复轮数 (repair_rounds)</label>
                        <input
                            v-model.number="config.settings.repair_rounds"
                            type="number"
                            min="0"
                            max="10"
                        />
                        <p class="hint">
                            生成后自动修复 JSON 校验错误的最大轮数，建议 2-4
                        </p>
                    </div>
                    <div class="field">
                        <label>并发数 (concurrency)</label>
                        <input
                            v-model.number="config.settings.concurrency"
                            type="number"
                            min="1"
                            max="8"
                        />
                        <p class="hint">
                            同时生成的幻灯片数量，建议 1（避免本地模型过载）
                        </p>
                    </div>
                </div>
            </div>

            <div class="settings-card">
                <h2 class="section-title">存储设置</h2>

                <div class="field-group">
                    <div class="field">
                        <label>数据目录</label>
                        <input
                            :value="appSettings.settings.data_dir"
                            readonly
                            class="dir-input"
                        />
                        <p class="hint">存放数据库和项目配置文件</p>
                    </div>

                    <div class="field">
                        <label>媒体目录</label>
                        <input
                            :value="appSettings.settings.media_dir"
                            readonly
                            class="dir-input"
                        />
                        <p class="hint">存放上传的图片和视频文件</p>
                    </div>
                </div>
            </div>

            <div class="settings-card">
                <h2 class="section-title">向量库状态</h2>

                <div class="embedding-status">
                    <div
                        class="status-indicator"
                        :class="{
                            ready: embeddingCacheReady,
                        }"
                    >
                        <span
                            v-if="embeddingCacheReady"
                            class="i-carbon:checkmark-circle"
                        />
                        <span v-else class="i-carbon:warning" />
                        <span>{{ embeddingStatusLabel }}</span>
                    </div>

                    <button
                        class="btn"
                        :disabled="
                            initializingEmbeddings ||
                            embeddingCacheReady
                        "
                        @click="initEmbeddings"
                    >
                        <span
                            v-if="initializingEmbeddings"
                            class="i-carbon:renew spin"
                        />
                        {{
                            initializingEmbeddings
                                ? embeddingStatus
                                : embeddingCacheReady
                                  ? "已就绪"
                                  : "开始初始化"
                        }}
                    </button>
                </div>

                <p
                    v-if="
                        appSettings.settings.initialized_embedding_model &&
                        appSettings.settings.initialized_embedding_model !==
                            config.settings.embedding.model
                    "
                    class="warning-notice"
                >
                    <span class="i-carbon:warning" />
                    当前向量模型为
                    <code>{{ config.settings.embedding.model }}</code
                    >，缓存仍对应
                    <code>{{
                        appSettings.settings.initialized_embedding_model
                    }}</code
                    >，需要重新初始化。
                </p>

                <p
                    v-if="!appSettings.settings.llm_configured"
                    class="warning-notice"
                >
                    <span class="i-carbon:warning" />
                    大模型未配置，向量库初始化需要先配置模型服务
                </p>
            </div>

            <div class="settings-card info-card">
                <h2 class="section-title">使用说明</h2>
                <ol class="guide-list">
                    <li>
                        文本生成、向量检索、图片理解现在各自独立配置，允许分别接入不同服务。
                    </li>
                    <li>
                        向量模型必须支持 <code>/v1/embeddings</code>，否则图标语义匹配和向量库初始化无法工作。
                    </li>
                    <li>
                        多模态模型用于图片分析与自动描述，需支持图像输入的
                        <code>/v1/chat/completions</code>。
                    </li>
                    <li>
                        保存设置后，在 Studio 主界面上传 Markdown 文件并点击「生成 Keynote」。
                    </li>
                </ol>
            </div>
        </div>
    </div>
</template>

<style scoped>
.settings-layout {
    display: flex;
    flex-direction: column;
    height: 100%;
    width: 100%;
    overflow: hidden;
    background: var(--studio-bg);
}

.top-bar {
    height: 44px;
    display: flex;
    align-items: center;
    gap: 0.75rem;
    padding: 0 1rem;
    border-bottom: 1px solid var(--studio-border);
    background: var(--studio-panel-2);
    flex-shrink: 0;
}

.page-title {
    display: flex;
    align-items: center;
    gap: 0.5rem;
    font-weight: 700;
    font-size: 0.92rem;
    flex: 1;
}

.settings-brand-mark {
    width: 24px;
    height: 24px;
    border-radius: 8px;
    display: inline-flex;
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
}

.settings-body {
    flex: 1;
    overflow-y: auto;
    padding: 1.5rem;
    display: flex;
    flex-direction: column;
    gap: 1rem;
    max-width: 980px;
    margin: 0 auto;
    width: 100%;
}

.settings-card {
    background: var(--studio-panel);
    border: 1px solid var(--studio-border);
    border-radius: 10px;
    padding: 1.25rem;
    display: flex;
    flex-direction: column;
    gap: 1rem;
}

.section-title {
    font-size: 0.9rem;
    font-weight: 600;
}

.section-desc,
.service-desc {
    font-size: 0.8rem;
    color: var(--studio-muted);
    line-height: 1.6;
}

.model-service-grid {
    display: grid;
    grid-template-columns: repeat(3, minmax(0, 1fr));
    gap: 0.9rem;
}

.service-panel {
    display: flex;
    flex-direction: column;
    gap: 0.9rem;
    padding: 1rem;
    border-radius: 12px;
    border: 1px solid var(--studio-border);
    background:
        linear-gradient(180deg, rgba(255, 255, 255, 0.02), transparent),
        var(--studio-panel-2);
}

.model-header {
    display: flex;
    align-items: flex-start;
    justify-content: space-between;
    gap: 0.75rem;
}

.model-header h3 {
    font-size: 0.88rem;
    font-weight: 600;
    margin: 0;
}

.field-group {
    display: flex;
    flex-direction: column;
    gap: 0.75rem;
}

.field-group.two-col {
    display: grid;
    grid-template-columns: 1fr 1fr;
    gap: 0.75rem;
}

.field {
    display: flex;
    flex-direction: column;
    gap: 0.3rem;
}

.select-row {
    display: flex;
    gap: 0.4rem;
}

.select-row select {
    flex: 1;
}

.select-row input {
    flex: 1.35;
}

.hint {
    font-size: 0.7rem;
    color: var(--studio-muted);
    line-height: 1.5;
}

.subtle {
    font-weight: 400;
    opacity: 0.5;
}

.error-msg {
    font-size: 0.75rem;
    color: var(--studio-error);
    background: var(--studio-error-bg);
    border: 1px solid var(--studio-error-border);
    border-radius: 6px;
    padding: 0.4rem 0.6rem;
}

.info-card {
    border-color: var(--studio-primary-border);
}

.guide-list {
    padding-left: 1.2rem;
    display: flex;
    flex-direction: column;
    gap: 0.4rem;
    font-size: 0.8rem;
    color: var(--studio-muted);
    line-height: 1.6;
}

.guide-list li code {
    background: var(--studio-surface);
    border: 1px solid var(--studio-border);
    border-radius: 3px;
    padding: 0 0.3rem;
    font-family: var(--font-mono);
    font-size: 0.8em;
    color: var(--studio-primary);
}

.spin {
    animation: spin 0.7s linear infinite;
}

@keyframes spin {
    to {
        transform: rotate(360deg);
    }
}

.dir-input {
    width: 100%;
    background: var(--studio-surface);
    border: 1px solid var(--studio-border);
    border-radius: 6px;
    padding: 0.5rem 0.75rem;
    color: var(--studio-text);
    font-size: 0.875rem;
    box-sizing: border-box;
}

.embedding-status {
    display: flex;
    align-items: center;
    gap: 1rem;
}

.status-indicator {
    display: flex;
    align-items: center;
    gap: 0.5rem;
    font-size: 0.875rem;
    color: var(--studio-muted);
}

.status-indicator.ready {
    color: var(--studio-primary);
}

.warning-notice {
    display: flex;
    align-items: center;
    gap: 0.5rem;
    margin-top: 0.75rem;
    padding: 0.5rem 0.75rem;
    background: var(--studio-surface);
    border: 1px solid var(--studio-border);
    border-radius: 6px;
    font-size: 0.8rem;
    color: var(--studio-muted);
}

@media (max-width: 980px) {
    .model-service-grid {
        grid-template-columns: 1fr;
    }
}

@media (max-width: 640px) {
    .settings-body {
        padding: 1rem;
    }

    .field-group.two-col,
    .select-row {
        grid-template-columns: 1fr;
        display: flex;
        flex-direction: column;
    }

    .embedding-status {
        align-items: flex-start;
        flex-direction: column;
    }
}
</style>
