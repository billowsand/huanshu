<script setup lang="ts">
import { ref, computed, onMounted } from "vue";
import { useRouter } from "vue-router";
import { invoke } from "@tauri-apps/api/core";
import { open } from "@tauri-apps/plugin-dialog";
import { useConfigStore } from "../stores/config";
import BrandMark from "../components/BrandMark.vue";

const router = useRouter();
const config = useConfigStore();

type Step = 1 | 2 | 3 | 4;

const currentStep = ref<Step>(1);
const dataDir = ref("");
const mediaDir = ref("");
const llmBaseUrl = ref("http://127.0.0.1:1234");
const llmApiKey = ref("");
const llmModel = ref("qwen/qwen3.5-9b");
const embeddingBaseUrl = ref("http://127.0.0.1:1234");
const embeddingApiKey = ref("");
const embeddingModel = ref("text-embedding-bge-m3");
const multimodalBaseUrl = ref("http://127.0.0.1:1234");
const multimodalApiKey = ref("");
const multimodalModel = ref("qwen/qwen2.5-vl-7b-instruct");
const embeddingProgress = ref(0);
const embeddingStatus = ref("");
const isInitializing = ref(false);
const skipLlm = ref(false);
const skipEmbedding = ref(false);
const modelError = ref("");

const defaultDataDir = ref("");
const defaultMediaDir = ref("");

onMounted(async () => {
    try {
        const { localDataDir } = await import("@tauri-apps/api/path");
        const localData = await localDataDir();
        defaultDataDir.value = `${localData}/auto-slidev-studio`;
        defaultMediaDir.value = `${localData}/auto-slidev-studio/media`;
        dataDir.value = defaultDataDir.value;
        mediaDir.value = defaultMediaDir.value;
    } catch (e) {
        defaultDataDir.value = "~/.local/share/auto-slidev-studio";
        defaultMediaDir.value = "~/.local/share/auto-slidev-studio/media";
        dataDir.value = defaultDataDir.value;
        mediaDir.value = defaultMediaDir.value;
    }
});

const canProceedStep1 = computed(
    () => dataDir.value.trim() && mediaDir.value.trim(),
);
const canProceedStep2 = computed(
    () =>
        skipLlm.value ||
        (llmBaseUrl.value.trim() &&
            llmModel.value.trim() &&
            embeddingBaseUrl.value.trim() &&
            embeddingModel.value.trim() &&
            multimodalBaseUrl.value.trim() &&
            multimodalModel.value.trim()),
);
const dataDirChanged = computed(
    () =>
        dataDir.value !== defaultDataDir.value ||
        mediaDir.value !== defaultMediaDir.value,
);
const embeddingsReady = computed(
    () =>
        !skipLlm.value &&
        !skipEmbedding.value &&
        embeddingProgress.value >= 100,
);

function nextStep() {
    if (currentStep.value === 1 && canProceedStep1.value) {
        if (skipLlm.value) {
            skipEmbedding.value = true;
            currentStep.value = 4;
        } else {
            currentStep.value = 2;
        }
    } else if (currentStep.value === 2 && canProceedStep2.value) {
        currentStep.value = 3;
    } else if (currentStep.value === 3) {
        currentStep.value = 4;
    }
}

function prevStep() {
    if (currentStep.value === 2) {
        currentStep.value = 1;
    } else if (currentStep.value === 3) {
        currentStep.value = 2;
    } else if (currentStep.value === 4) {
        currentStep.value = skipLlm.value ? 1 : 3;
    }
}

async function browseDataDir() {
    const selected = await open({ directory: true });
    if (selected) {
        dataDir.value = selected as string;
        if (!mediaDir.value.includes("/projects/")) {
            mediaDir.value = `${selected}/media`;
        }
    }
}

async function browseMediaDir() {
    const selected = await open({ directory: true });
    if (selected) {
        mediaDir.value = selected as string;
    }
}

async function verifyModels() {
    modelError.value = "";
    try {
        await invoke<string[]>("list_models", { target: "llm" });
        await invoke<string[]>("list_models", { target: "embedding" });
        await invoke<string[]>("list_models", { target: "multimodal" });
    } catch (e) {
        modelError.value = String(e);
    }
}

async function startEmbeddingInit() {
    isInitializing.value = true;
    embeddingProgress.value = 0;
    embeddingStatus.value = "正在初始化图标向量库...";

    try {
        config.settings.llm.base_url = llmBaseUrl.value;
        config.settings.llm.api_key = llmApiKey.value;
        config.settings.llm.model = llmModel.value;
        config.settings.embedding.base_url = embeddingBaseUrl.value;
        config.settings.embedding.api_key = embeddingApiKey.value;
        config.settings.embedding.model = embeddingModel.value;
        config.settings.multimodal.base_url = multimodalBaseUrl.value;
        config.settings.multimodal.api_key = multimodalApiKey.value;
        config.settings.multimodal.model = multimodalModel.value;
        await config.save();

        await invoke("ensure_icon_embeddings");

        embeddingProgress.value = 100;
        embeddingStatus.value = "初始化完成！";
    } catch (e) {
        embeddingStatus.value = `初始化失败: ${e}`;
        modelError.value = String(e);
    } finally {
        isInitializing.value = false;
    }
}

async function finishWizard() {
    try {
        await invoke("save_app_settings", {
            settings: {
                data_dir: dataDir.value,
                media_dir: mediaDir.value,
                llm_configured: !skipLlm.value,
                embeddings_ready: embeddingsReady.value,
                initialized_embedding_model:
                    !skipLlm.value && !skipEmbedding.value
                        ? embeddingModel.value
                        : "",
            },
        });

        if (!skipLlm.value) {
            config.settings.llm.base_url = llmBaseUrl.value;
            config.settings.llm.api_key = llmApiKey.value;
            config.settings.llm.model = llmModel.value;
            config.settings.embedding.base_url = embeddingBaseUrl.value;
            config.settings.embedding.api_key = embeddingApiKey.value;
            config.settings.embedding.model = embeddingModel.value;
            config.settings.multimodal.base_url = multimodalBaseUrl.value;
            config.settings.multimodal.api_key = multimodalApiKey.value;
            config.settings.multimodal.model = multimodalModel.value;
            await config.save();
        }

        await invoke("complete_first_run", { llmConfigured: !skipLlm.value });

        router.push("/");
    } catch (e) {
        modelError.value = String(e);
    }
}
</script>

<template>
    <div class="wizard-layout">
        <div class="wizard-card">
            <div class="wizard-header">
                <div class="wizard-logo">
                    <span class="wizard-logo-mark">
                        <BrandMark :size="28" />
                    </span>
                    <div class="wizard-logo-copy">
                        <span class="wizard-logo-kicker">HUANSHU STUDIO</span>
                        <span class="wizard-logo-title">幻述</span>
                    </div>
                </div>
                <h1 class="wizard-title">首次启动配置</h1>
                <p class="wizard-subtitle">
                    完成以下设置后，即可开始生成和演示你的幻灯片
                </p>
            </div>

            <div class="wizard-steps-indicator">
                <div
                    class="step-dot"
                    :class="{
                        active: currentStep === 1,
                        done: currentStep > 1,
                    }"
                >
                    1
                </div>
                <div class="step-line" :class="{ done: currentStep > 1 }" />
                <div
                    class="step-dot"
                    :class="{
                        active: currentStep === 2,
                        done: currentStep > 2,
                        skipped: skipLlm,
                    }"
                >
                    2
                </div>
                <div
                    class="step-line"
                    :class="{ done: currentStep > 2 || skipLlm }"
                />
                <div
                    class="step-dot"
                    :class="{
                        active: currentStep === 3,
                        done: currentStep > 3,
                        skipped: skipEmbedding || skipLlm,
                    }"
                >
                    3
                </div>
                <div
                    class="step-line"
                    :class="{ done: currentStep > 3 || skipLlm }"
                />
                <div
                    class="step-dot"
                    :class="{ active: currentStep === 4, done: false }"
                >
                    4
                </div>
            </div>

            <!-- Step 1: Storage Directories -->
            <div v-if="currentStep === 1" class="wizard-step">
                <h2 class="step-title">选择存储目录</h2>
                <p class="step-desc">设置数据库和多媒体文件的存储位置</p>

                <div class="field-group">
                    <div class="field">
                        <label>数据目录</label>
                        <div class="dir-input-row">
                            <input
                                v-model="dataDir"
                                readonly
                                class="dir-input"
                            />
                            <button
                                class="btn btn-ghost"
                                @click="browseDataDir"
                            >
                                浏览...
                            </button>
                        </div>
                        <p class="field-hint">存放数据库和项目配置文件</p>
                    </div>

                    <div class="field">
                        <label>媒体目录</label>
                        <div class="dir-input-row">
                            <input
                                v-model="mediaDir"
                                readonly
                                class="dir-input"
                            />
                            <button
                                class="btn btn-ghost"
                                @click="browseMediaDir"
                            >
                                浏览...
                            </button>
                        </div>
                        <p class="field-hint">存放上传的图片和视频文件</p>
                    </div>
                </div>
            </div>

            <!-- Step 2: LLM Configuration -->
            <div v-if="currentStep === 2" class="wizard-step">
                <h2 class="step-title">配置大模型</h2>
                <p class="step-desc">
                    设置 AI 生成所需的模型服务（可跳过，仅播放幻灯片时无需配置）
                </p>

                <div class="skip-notice">
                    <span class="i-carbon:information" />
                    跳过此步骤仍可使用幻灯片播放功能，但无法生成新幻灯片
                </div>

                <div class="field-group">
                    <div class="field">
                        <label>大语言模型 API 地址</label>
                        <input
                            v-model="llmBaseUrl"
                            placeholder="http://127.0.0.1:1234"
                        />
                    </div>

                    <div class="field">
                        <label
                            >API Key
                            <span style="opacity: 0.5"
                                >(本地服务留空)</span
                            ></label
                        >
                        <input
                            v-model="llmApiKey"
                            type="password"
                            placeholder="sk-..."
                            autocomplete="off"
                        />
                    </div>

                    <div class="field">
                        <label>大语言模型</label>
                        <input
                            v-model="llmModel"
                            placeholder="qwen/qwen3.5-9b"
                        />
                    </div>

                    <div class="field">
                        <label>向量模型 API 地址</label>
                        <input
                            v-model="embeddingBaseUrl"
                            placeholder="http://127.0.0.1:1234"
                        />
                    </div>

                    <div class="field">
                        <label>向量模型 API Key</label>
                        <input
                            v-model="embeddingApiKey"
                            type="password"
                            placeholder="sk-..."
                            autocomplete="off"
                        />
                    </div>

                    <div class="field">
                        <label>向量模型</label>
                        <input
                            v-model="embeddingModel"
                            placeholder="text-embedding-bge-m3"
                        />
                    </div>

                    <div class="field">
                        <label>多模态模型 API 地址</label>
                        <input
                            v-model="multimodalBaseUrl"
                            placeholder="http://127.0.0.1:1234"
                        />
                    </div>

                    <div class="field">
                        <label>多模态模型 API Key</label>
                        <input
                            v-model="multimodalApiKey"
                            type="password"
                            placeholder="sk-..."
                            autocomplete="off"
                        />
                    </div>

                    <div class="field">
                        <label>多模态模型</label>
                        <input
                            v-model="multimodalModel"
                            placeholder="qwen/qwen2.5-vl-7b-instruct"
                        />
                    </div>
                </div>

                <p v-if="modelError" class="error-msg">{{ modelError }}</p>
            </div>

            <!-- Step 3: Embedding Initialization -->
            <div v-if="currentStep === 3" class="wizard-step">
                <h2 class="step-title">初始化向量库</h2>
                <p class="step-desc">
                    正在为图标语义匹配生成向量索引（需要一些时间）
                </p>

                <div class="embedding-progress">
                    <div class="progress-bar">
                        <div
                            class="progress-fill"
                            :style="{ width: embeddingProgress + '%' }"
                        />
                    </div>
                    <p class="progress-status">{{ embeddingStatus }}</p>
                </div>

                <button
                    class="btn btn-primary"
                    :disabled="isInitializing"
                    @click="startEmbeddingInit"
                >
                    <span v-if="isInitializing" class="i-carbon:renew spin" />
                    {{ isInitializing ? "初始化中..." : "开始初始化" }}
                </button>

                <button
                    class="btn btn-ghost skip-btn"
                    @click="
                        skipEmbedding = true;
                        currentStep = 4;
                    "
                >
                    跳过（可在设置中后续初始化）
                </button>
            </div>

            <!-- Step 4: Done -->
            <div v-if="currentStep === 4" class="wizard-step">
                <div class="done-icon">
                    <span class="done-icon-badge">
                        <BrandMark :size="52" />
                    </span>
                </div>
                <h2 class="step-title">配置完成！</h2>

                <div class="done-summary">
                    <div class="summary-row">
                        <span class="summary-label">数据目录</span>
                        <span class="summary-value">{{ dataDir }}</span>
                    </div>
                    <div class="summary-row">
                        <span class="summary-label">媒体目录</span>
                        <span class="summary-value">{{ mediaDir }}</span>
                    </div>
                    <div class="summary-row">
                        <span class="summary-label">大模型</span>
                        <span class="summary-value">{{
                            skipLlm ? "未配置" : llmModel
                        }}</span>
                    </div>
                    <div class="summary-row">
                        <span class="summary-label">向量模型</span>
                        <span class="summary-value">{{
                            skipLlm ? "未配置" : embeddingModel
                        }}</span>
                    </div>
                    <div class="summary-row">
                        <span class="summary-label">多模态模型</span>
                        <span class="summary-value">{{
                            skipLlm ? "未配置" : multimodalModel
                        }}</span>
                    </div>
                    <div class="summary-row">
                        <span class="summary-label">向量库</span>
                        <span class="summary-value">{{
                            skipEmbedding || skipLlm ? "未初始化" : "已就绪"
                        }}</span>
                    </div>
                </div>

                <div v-if="dataDirChanged" class="restart-warning">
                    <span class="i-carbon:warning" />
                    <div>
                        <strong>需要重启应用</strong>
                        <p>
                            数据目录已更改，请关闭并重新启动应用以使更改生效。当前会话中的新建项目将存储在原目录。
                        </p>
                    </div>
                </div>
            </div>

            <div class="wizard-footer">
                <button
                    v-if="currentStep > 1"
                    class="btn btn-ghost"
                    @click="prevStep"
                >
                    <span class="i-carbon:arrow-left" /> 上一步
                </button>
                <div class="spacer" />
                <button
                    v-if="currentStep === 1 && canProceedStep1"
                    class="btn btn-primary"
                    @click="nextStep"
                >
                    下一步 <span class="i-carbon:arrow-right" />
                </button>
                <button
                    v-if="currentStep === 2"
                    class="btn btn-ghost"
                    @click="
                        skipLlm = true;
                        skipEmbedding = true;
                        currentStep = 4;
                    "
                >
                    跳过
                </button>
                <button
                    v-if="currentStep === 2 && canProceedStep2"
                    class="btn btn-primary"
                    @click="nextStep"
                >
                    下一步 <span class="i-carbon:arrow-right" />
                </button>
                <button
                    v-if="currentStep === 3"
                    class="btn btn-primary"
                    :disabled="!skipEmbedding && embeddingProgress < 100"
                    @click="nextStep"
                >
                    下一步 <span class="i-carbon:arrow-right" />
                </button>
                <button
                    v-if="currentStep === 4"
                    class="btn btn-primary pulse-btn"
                    @click="finishWizard"
                >
                    进入 Studio <span class="i-carbon:arrow-right" />
                </button>
            </div>
        </div>
    </div>
</template>

<style scoped>
.wizard-layout {
    display: flex;
    align-items: center;
    justify-content: center;
    min-height: 100%;
    background: var(--studio-bg);
    padding: 2rem;
}

.wizard-card {
    background: var(--studio-panel);
    border: 1px solid var(--studio-border);
    border-radius: 16px;
    padding: 2.5rem;
    max-width: 560px;
    width: 100%;
}

.wizard-header {
    text-align: center;
    margin-bottom: 2rem;
}

.wizard-logo {
    display: inline-flex;
    align-items: center;
    gap: 0.9rem;
    margin-bottom: 1rem;
    text-align: left;
}

.wizard-logo-mark {
    width: 56px;
    height: 56px;
    border-radius: 18px;
    display: inline-flex;
    align-items: center;
    justify-content: center;
    background:
        radial-gradient(
            circle at 30% 25%,
            rgba(255, 213, 140, 0.2),
            transparent 55%
        ),
        linear-gradient(145deg, rgba(23, 34, 51, 0.98), rgba(10, 18, 29, 0.98));
    border: 1px solid rgba(110, 160, 210, 0.14);
    box-shadow: 0 20px 45px rgba(0, 0, 0, 0.2);
}

.wizard-logo-copy {
    display: flex;
    flex-direction: column;
    gap: 0.15rem;
}

.wizard-logo-kicker {
    font-size: 0.68rem;
    font-weight: 700;
    letter-spacing: 0.16em;
    color: var(--studio-primary);
}

.wizard-logo-title {
    font-size: 1.6rem;
    font-weight: 800;
    letter-spacing: 0.08em;
    color: var(--studio-text);
}

.wizard-title {
    font-size: 1.5rem;
    font-weight: 600;
    margin: 0 0 0.5rem;
}

.wizard-subtitle {
    color: var(--studio-muted);
    font-size: 0.875rem;
    margin: 0;
}

.wizard-steps-indicator {
    display: flex;
    align-items: center;
    justify-content: center;
    gap: 0;
    margin-bottom: 2rem;
}

.step-dot {
    width: 32px;
    height: 32px;
    border-radius: 50%;
    display: flex;
    align-items: center;
    justify-content: center;
    font-size: 0.8rem;
    font-weight: 600;
    background: var(--studio-surface);
    border: 2px solid var(--studio-border);
    color: var(--studio-muted);
    transition: all 0.3s;
}

.step-dot.active {
    background: var(--studio-primary-bg);
    border-color: var(--studio-primary);
    color: var(--studio-primary);
}

.step-dot.done {
    background: var(--studio-primary);
    border-color: var(--studio-primary);
    color: white;
}

.step-dot.skipped {
    background: var(--studio-surface);
    border-color: var(--studio-muted);
    color: var(--studio-muted);
}

.step-line {
    width: 48px;
    height: 2px;
    background: var(--studio-border);
    transition: background 0.3s;
}

.step-line.done {
    background: var(--studio-primary);
}

.wizard-step {
    min-height: 300px;
}

.step-title {
    font-size: 1.25rem;
    font-weight: 600;
    margin: 0 0 0.5rem;
}

.step-desc {
    color: var(--studio-muted);
    font-size: 0.875rem;
    margin: 0 0 1.5rem;
}

.field-group {
    display: flex;
    flex-direction: column;
    gap: 1rem;
}

.field {
    display: flex;
    flex-direction: column;
    gap: 0.3rem;
}

.field label {
    font-size: 0.8rem;
    font-weight: 500;
}

.dir-input-row {
    display: flex;
    gap: 0.5rem;
}

.dir-input {
    flex: 1;
    background: var(--studio-surface);
    border: 1px solid var(--studio-border);
    border-radius: 6px;
    padding: 0.5rem 0.75rem;
    color: var(--studio-text);
    font-size: 0.875rem;
}

.field-hint {
    font-size: 0.7rem;
    color: var(--studio-muted);
    margin: 0;
}

.skip-notice {
    display: flex;
    align-items: center;
    gap: 0.5rem;
    padding: 0.75rem;
    background: var(--studio-surface);
    border: 1px solid var(--studio-border);
    border-radius: 8px;
    font-size: 0.8rem;
    color: var(--studio-muted);
    margin-bottom: 1.5rem;
}

.embedding-progress {
    margin-bottom: 1.5rem;
}

.progress-bar {
    height: 8px;
    background: var(--studio-surface);
    border-radius: 4px;
    overflow: hidden;
    margin-bottom: 0.75rem;
}

.progress-fill {
    height: 100%;
    background: var(--studio-primary);
    transition: width 0.3s;
}

.progress-status {
    font-size: 0.875rem;
    color: var(--studio-muted);
    margin: 0;
    text-align: center;
}

.skip-btn {
    margin-top: 0.75rem;
    width: 100%;
}

.done-icon {
    text-align: center;
    margin-bottom: 1rem;
}

.done-icon-badge {
    width: 88px;
    height: 88px;
    border-radius: 28px;
    display: inline-flex;
    align-items: center;
    justify-content: center;
    background:
        radial-gradient(
            circle at 30% 25%,
            rgba(255, 213, 140, 0.2),
            transparent 55%
        ),
        linear-gradient(145deg, rgba(23, 34, 51, 0.98), rgba(10, 18, 29, 0.98));
    border: 1px solid rgba(110, 160, 210, 0.14);
    box-shadow: 0 24px 50px rgba(0, 0, 0, 0.22);
}

.done-summary {
    background: var(--studio-surface);
    border: 1px solid var(--studio-border);
    border-radius: 8px;
    padding: 1rem;
    display: flex;
    flex-direction: column;
    gap: 0.5rem;
}

.summary-row {
    display: flex;
    justify-content: space-between;
    font-size: 0.875rem;
}

.summary-label {
    color: var(--studio-muted);
}

.summary-value {
    color: var(--studio-text);
    font-weight: 500;
    max-width: 280px;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
}

.error-msg {
    margin-top: 1rem;
    padding: 0.75rem;
    background: var(--studio-error-bg);
    border: 1px solid var(--studio-error-border);
    border-radius: 6px;
    color: var(--studio-error);
    font-size: 0.8rem;
}

.restart-warning {
    margin-top: 1.5rem;
    padding: 1rem;
    background: var(--studio-warning-bg, #fef3c7);
    border: 1px solid var(--studio-warning-border, #f59e0b);
    border-radius: 8px;
    display: flex;
    gap: 0.75rem;
    align-items: flex-start;
    font-size: 0.85rem;
}

.restart-warning strong {
    color: var(--studio-warning, #92400e);
}

.restart-warning p {
    margin: 0.25rem 0 0;
    color: var(--studio-muted);
}

.wizard-footer {
    display: flex;
    align-items: center;
    margin-top: 2rem;
    padding-top: 1.5rem;
    border-top: 1px solid var(--studio-border);
}

.spacer {
    flex: 1;
}

.pulse-btn {
    animation: pulse-glow 3s ease-in-out infinite;
}

@keyframes pulse-glow {
    0%,
    100% {
        box-shadow: 0 0 0 0 transparent;
    }
    50% {
        box-shadow: 0 0 0 6px var(--studio-primary-bg);
    }
}

.spin {
    animation: spin 0.7s linear infinite;
}
@keyframes spin {
    to {
        transform: rotate(360deg);
    }
}
</style>
