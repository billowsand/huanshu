<script setup lang="ts">
import { ref, onMounted } from "vue";
import { useRouter } from "vue-router";
import { useConfigStore } from "../stores/config";
import { useAppSettingsStore } from "../stores/appSettings";
import { invoke } from "@tauri-apps/api/core";
import BrandMark from "../components/BrandMark.vue";

const router = useRouter();
const config = useConfigStore();
const appSettings = useAppSettingsStore();

const models = ref<string[]>([]);
const loadingModels = ref(false);
const modelError = ref("");
const saved = ref(false);
const initializingEmbeddings = ref(false);
const embeddingStatus = ref("");

onMounted(async () => {
    await config.load();
    await appSettings.load();
});

async function fetchModels() {
    loadingModels.value = true;
    modelError.value = "";
    try {
        models.value = await config.listModels();
    } catch (e: unknown) {
        modelError.value = String(e);
    } finally {
        loadingModels.value = false;
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
        appSettings.settings.embeddings_ready = true;
        await appSettings.save();
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
                <h2 class="section-title">LLM API 连接（OpenAI 兼容）</h2>
                <p class="section-desc">
                    支持任何 OpenAI 兼容接口：LM
                    Studio、Ollama、OpenAI、DeepSeek、SiliconFlow
                    等。本地服务无需填写 API Key。
                </p>

                <div class="field-group">
                    <div class="field">
                        <label>API 地址</label>
                        <input
                            v-model="config.settings.base_url"
                            placeholder="http://127.0.0.1:1234"
                        />
                        <p class="hint">
                            LM Studio 默认：http://127.0.0.1:1234 · Ollama
                            默认：http://127.0.0.1:11434 ·
                            OpenAI：https://api.openai.com
                        </p>
                    </div>
                    <div class="field">
                        <label
                            >API Key
                            <span style="font-weight: 400; opacity: 0.5"
                                >（本地服务留空）</span
                            ></label
                        >
                        <input
                            v-model="config.settings.api_key"
                            type="password"
                            placeholder="sk-..."
                            autocomplete="off"
                        />
                    </div>
                </div>

                <div class="model-section">
                    <div class="model-header">
                        <h3>模型选择</h3>
                        <button
                            class="btn"
                            :disabled="loadingModels"
                            @click="fetchModels"
                        >
                            <span
                                v-if="loadingModels"
                                class="i-carbon:renew spin"
                            />
                            <span v-else class="i-carbon:refresh" />
                            {{ loadingModels ? "加载中..." : "获取可用模型" }}
                        </button>
                    </div>
                    <p v-if="modelError" class="error-msg">{{ modelError }}</p>

                    <div class="field-group">
                        <div class="field">
                            <label>生成模型</label>
                            <div class="select-row">
                                <select v-model="config.settings.model">
                                    <option
                                        v-for="m in models"
                                        :key="m"
                                        :value="m"
                                    >
                                        {{ m }}
                                    </option>
                                    <option :value="config.settings.model">
                                        {{ config.settings.model }} (当前)
                                    </option>
                                </select>
                                <input
                                    v-model="config.settings.model"
                                    placeholder="或手动输入模型 ID"
                                />
                            </div>
                            <p class="hint">
                                用于生成幻灯片内容，推荐 qwen/qwen3.5-9b
                                或更大的模型
                            </p>
                        </div>

                        <div class="field">
                            <label>Embedding 模型</label>
                            <div class="select-row">
                                <select
                                    v-model="config.settings.embedding_model"
                                >
                                    <option
                                        v-for="m in models"
                                        :key="m"
                                        :value="m"
                                    >
                                        {{ m }}
                                    </option>
                                    <option
                                        :value="config.settings.embedding_model"
                                    >
                                        {{
                                            config.settings.embedding_model
                                        }}
                                        (当前)
                                    </option>
                                </select>
                                <input
                                    v-model="config.settings.embedding_model"
                                    placeholder="或手动输入模型 ID"
                                />
                            </div>
                            <p class="hint">
                                用于图标语义匹配，推荐 text-embedding-bge-m3
                            </p>
                        </div>
                    </div>
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
                            同时生成的幻灯片数量，建议 1（避免 LM Studio 过载）
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
                            ready: appSettings.settings.embeddings_ready,
                        }"
                    >
                        <span
                            v-if="appSettings.settings.embeddings_ready"
                            class="i-carbon:checkmark-circle"
                        />
                        <span v-else class="i-carbon:warning" />
                        <span>{{
                            appSettings.settings.embeddings_ready
                                ? "已初始化"
                                : "未初始化"
                        }}</span>
                    </div>

                    <button
                        class="btn"
                        :disabled="
                            initializingEmbeddings ||
                            appSettings.settings.embeddings_ready
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
                                : appSettings.settings.embeddings_ready
                                  ? "已就绪"
                                  : "重新初始化"
                        }}
                    </button>
                </div>

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
                        <b>本地模型（LM Studio / Ollama）</b>：填写本地地址，API
                        Key 留空，点击「获取可用模型」验证连接
                    </li>
                    <li>
                        <b>云端 API（OpenAI / DeepSeek / SiliconFlow）</b
                        >：填写对应 Base URL 和 API Key，手动输入模型名称
                    </li>
                    <li>
                        注意：Embedding 模型需要支持
                        <code>/v1/embeddings</code>，用于图标语义匹配；云端 API
                        可填同一模型
                    </li>
                    <li>
                        保存设置后，在 Studio 主界面上传 Markdown
                        文件并点击「生成 Keynote」
                    </li>
                    <li>
                        生成完成后点击「全屏演示」查看幻灯片，不需要启动任何服务
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
    max-width: 720px;
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

.section-desc {
    font-size: 0.8rem;
    color: var(--studio-muted);
    line-height: 1.6;
}

.model-section {
    display: flex;
    flex-direction: column;
    gap: 0.75rem;
}
.model-header {
    display: flex;
    align-items: center;
    justify-content: space-between;
}
.model-header h3 {
    font-size: 0.85rem;
    font-weight: 500;
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
    flex: 1.5;
}

.hint {
    font-size: 0.7rem;
    color: var(--studio-muted);
    line-height: 1.5;
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
</style>
