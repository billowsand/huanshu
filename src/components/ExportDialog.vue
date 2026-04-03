<script setup lang="ts">
import { computed, ref, watch } from "vue";

const props = defineProps<{
  visible: boolean;
  projectName?: string;
}>();

const emit = defineEmits<{
  (e: "close"): void;
  (e: "submit", password: string | null): void;
}>();

const encrypt = ref(false);
const password = ref("");
const confirmPassword = ref("");
const showPassword = ref(false);
const error = ref("");

const strength = computed(() => {
  const p = password.value;
  if (p.length === 0) return { score: 0, label: "", color: "" };

  let score = 0;
  if (p.length >= 8) score++;
  if (p.length >= 12) score++;
  if (/[a-z]/.test(p) && /[A-Z]/.test(p)) score++;
  if (/\d/.test(p)) score++;
  if (/[^a-zA-Z0-9]/.test(p)) score++;

  const levels = [
    { label: "太弱", color: "#ef4444" },
    { label: "弱", color: "#f97316" },
    { label: "一般", color: "#eab308" },
    { label: "良好", color: "#22c55e" },
    { label: "强", color: "#10b981" },
  ];
  return {
    score,
    label: levels[Math.min(score, 4)].label,
    color: levels[Math.min(score, 4)].color,
  };
});

const canSubmit = computed(() => {
  if (!encrypt.value) return true;
  return password.value.length >= 8 && password.value === confirmPassword.value;
});

watch(
  () => props.visible,
  (visible) => {
    if (!visible) return;
    encrypt.value = false;
    password.value = "";
    confirmPassword.value = "";
    showPassword.value = false;
    error.value = "";
  },
);

watch(encrypt, (enabled) => {
  if (!enabled) {
    password.value = "";
    confirmPassword.value = "";
    showPassword.value = false;
    error.value = "";
  }
});

function handleSubmit() {
  if (!canSubmit.value) return;

  if (!encrypt.value) {
    emit("submit", null);
    return;
  }

  if (!/(?=.*[a-z])(?=.*[A-Z])(?=.*\d).{8,}/.test(password.value)) {
    error.value = "密码必须至少8字符，包含大小写字母和数字";
    return;
  }

  emit("submit", password.value);
}

function handleClose() {
  emit("close");
}
</script>

<template>
  <Teleport to="body">
    <Transition name="modal">
      <div v-if="visible" class="modal-overlay" @click.self="handleClose">
        <div class="modal-box export-modal">
          <button class="modal-close" @click="handleClose">
            <span class="i-carbon:close" />
          </button>

          <div class="modal-icon">
            <span class="i-carbon:download" />
          </div>

          <h3 class="modal-title">导出演示文稿</h3>
          <p class="modal-desc">
            <template v-if="projectName">准备导出「{{ projectName }}」</template>
            <template v-else>选择导出方式并确认</template>
          </p>

          <div class="option-grid">
            <button
              class="option-card"
              :class="{ active: !encrypt }"
              @click="encrypt = false"
            >
              <span class="option-icon i-carbon:document-export" />
              <span class="option-title">普通导出</span>
              <span class="option-copy">直接生成 `.keynn` 文件</span>
            </button>
            <button
              class="option-card"
              :class="{ active: encrypt }"
              @click="encrypt = true"
            >
              <span class="option-icon i-carbon:locked" />
              <span class="option-title">加密导出</span>
              <span class="option-copy">设置密码后再生成导出文件</span>
            </button>
          </div>

          <div v-if="encrypt" class="encrypt-panel">
            <div class="form-group">
              <label class="form-label">导出密码</label>
              <div class="input-wrapper">
                <input
                  v-model="password"
                  :type="showPassword ? 'text' : 'password'"
                  class="form-input"
                  placeholder="设置密码"
                  @keyup.enter="canSubmit && handleSubmit()"
                />
                <button class="toggle-visibility" @click="showPassword = !showPassword">
                  <span :class="showPassword ? 'i-carbon:view-off' : 'i-carbon:view'" />
                </button>
              </div>
              <div class="strength-bar">
                <div class="strength-track">
                  <div
                    class="strength-fill"
                    :style="{ width: `${(strength.score / 5) * 100}%`, background: strength.color }"
                  />
                </div>
                <span class="strength-label" :style="{ color: strength.color }">
                  {{ strength.label }}
                </span>
              </div>
            </div>

            <div class="form-group">
              <label class="form-label">确认密码</label>
              <div class="input-wrapper">
                <input
                  v-model="confirmPassword"
                  :type="showPassword ? 'text' : 'password'"
                  class="form-input"
                  :class="{ error: confirmPassword.length > 0 && password !== confirmPassword }"
                  placeholder="再次输入密码"
                  @keyup.enter="canSubmit && handleSubmit()"
                />
              </div>
              <span
                v-if="confirmPassword.length > 0 && password !== confirmPassword"
                class="field-error"
              >
                两次输入的密码不一致
              </span>
            </div>

            <p class="hint">
              密码至少 8 位，需包含大写字母、小写字母和数字。
            </p>
          </div>

          <p v-if="error" class="submit-error">{{ error }}</p>

          <div class="modal-actions">
            <button class="btn btn-ghost" @click="handleClose">取消</button>
            <button class="btn btn-primary" :disabled="!canSubmit" @click="handleSubmit">
              {{ encrypt ? "加密导出" : "开始导出" }}
            </button>
          </div>
        </div>
      </div>
    </Transition>
  </Teleport>
</template>

<style scoped>
.modal-overlay {
  position: fixed;
  inset: 0;
  background: rgba(0, 0, 0, 0.5);
  display: flex;
  align-items: center;
  justify-content: center;
  z-index: 1000;
}

.modal-box {
  background: var(--studio-panel);
  border: 1px solid var(--studio-border);
  border-radius: 14px;
  padding: 24px;
  min-width: 520px;
  max-width: 92vw;
  position: relative;
  box-shadow: 0 20px 60px rgba(0, 0, 0, 0.35);
}

.export-modal {
  text-align: center;
}

.modal-close {
  position: absolute;
  top: 12px;
  right: 12px;
  background: none;
  border: none;
  cursor: pointer;
  padding: 4px;
  color: var(--studio-muted);
  border-radius: 4px;
}

.modal-close:hover {
  background: var(--studio-border);
  color: var(--studio-text);
}

.modal-icon {
  width: 52px;
  height: 52px;
  border-radius: 50%;
  display: flex;
  align-items: center;
  justify-content: center;
  margin: 0 auto 16px;
  font-size: 24px;
  color: #f59e0b;
  background: rgba(245, 158, 11, 0.14);
}

.modal-title {
  font-size: 18px;
  font-weight: 700;
  margin: 0 0 8px;
  color: var(--studio-text);
}

.modal-desc {
  font-size: 14px;
  color: var(--studio-muted);
  margin: 0 0 20px;
}

.option-grid {
  display: grid;
  grid-template-columns: repeat(2, minmax(0, 1fr));
  gap: 12px;
  margin-bottom: 18px;
}

.option-card {
  display: flex;
  flex-direction: column;
  align-items: flex-start;
  gap: 6px;
  padding: 16px;
  border-radius: 12px;
  border: 1px solid var(--studio-border);
  background: var(--studio-surface);
  color: var(--studio-text);
  text-align: left;
  cursor: pointer;
  transition: border-color 0.15s ease, transform 0.15s ease, background 0.15s ease;
}

.option-card:hover {
  border-color: var(--studio-primary-border);
  transform: translateY(-1px);
}

.option-card.active {
  border-color: var(--studio-primary-border);
  background: var(--studio-primary-bg);
  box-shadow: inset 0 0 0 1px rgba(120, 170, 220, 0.18);
}

.option-icon {
  font-size: 20px;
  color: var(--studio-primary);
}

.option-title {
  font-size: 14px;
  font-weight: 700;
}

.option-copy {
  font-size: 12px;
  line-height: 1.5;
  color: var(--studio-muted);
}

.encrypt-panel {
  margin-top: 4px;
  padding: 18px;
  border-radius: 12px;
  border: 1px solid var(--studio-border);
  background: linear-gradient(180deg, rgba(120, 170, 220, 0.08), rgba(120, 170, 220, 0.03));
}

.form-group {
  text-align: left;
  margin-bottom: 16px;
}

.form-label {
  display: block;
  font-size: 13px;
  font-weight: 600;
  color: var(--studio-text);
  margin-bottom: 6px;
}

.input-wrapper {
  position: relative;
  display: flex;
  align-items: center;
}

.form-input {
  width: 100%;
  padding: 10px 40px 10px 12px;
  border: 1px solid var(--studio-border);
  border-radius: 10px;
  background: var(--studio-panel);
  color: var(--studio-text);
  font-size: 14px;
  outline: none;
}

.form-input:focus {
  border-color: var(--studio-primary-border);
  box-shadow: 0 0 0 3px rgba(120, 170, 220, 0.12);
}

.form-input.error {
  border-color: var(--studio-error-border);
}

.toggle-visibility {
  position: absolute;
  right: 10px;
  background: none;
  border: none;
  color: var(--studio-muted);
  cursor: pointer;
  display: inline-flex;
  align-items: center;
  justify-content: center;
}

.strength-bar {
  display: flex;
  align-items: center;
  gap: 10px;
  margin-top: 10px;
}

.strength-track {
  flex: 1;
  height: 6px;
  border-radius: 999px;
  background: rgba(255, 255, 255, 0.08);
  overflow: hidden;
}

.strength-fill {
  height: 100%;
  border-radius: inherit;
  transition: width 0.2s ease;
}

.strength-label {
  min-width: 28px;
  font-size: 12px;
  font-weight: 600;
}

.field-error {
  display: inline-block;
  margin-top: 6px;
  font-size: 12px;
  color: var(--studio-error);
}

.hint {
  margin: 0;
  text-align: left;
  font-size: 12px;
  line-height: 1.5;
  color: var(--studio-muted);
}

.submit-error {
  margin: 14px 0 0;
  font-size: 13px;
  color: var(--studio-error);
}

.modal-actions {
  display: flex;
  justify-content: center;
  gap: 12px;
  margin-top: 22px;
}

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
  transform: scale(0.96);
  opacity: 0;
}

@media (max-width: 640px) {
  .modal-box {
    min-width: 0;
    width: calc(100vw - 24px);
    padding: 20px;
  }

  .option-grid {
    grid-template-columns: 1fr;
  }
}
</style>
