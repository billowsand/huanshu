<script setup lang="ts">
import { ref, computed, watch } from 'vue'

const props = defineProps<{
  visible: boolean
  mode: 'encrypt' | 'decrypt'
  projectName?: string
  externalError?: string
}>()

const emit = defineEmits<{
  (e: 'close'): void
  (e: 'submit', password: string): void
}>()

const password = ref('')
const confirmPassword = ref('')
const showPassword = ref(false)
const internalError = ref('')

const error = computed(() => internalError.value || props.externalError || '')

const strength = computed(() => {
  const p = password.value
  if (p.length === 0) return { score: 0, label: '', color: '' }
  let score = 0
  if (p.length >= 8) score++
  if (p.length >= 12) score++
  if (/[a-z]/.test(p) && /[A-Z]/.test(p)) score++
  if (/\d/.test(p)) score++
  if (/[^a-zA-Z0-9]/.test(p)) score++

  const levels = [
    { label: '太弱', color: '#ef4444' },
    { label: '弱', color: '#f97316' },
    { label: '一般', color: '#eab308' },
    { label: '良好', color: '#22c55e' },
    { label: '强', color: '#10b981' },
  ]
  return { score, label: levels[Math.min(score, 4)].label, color: levels[Math.min(score, 4)].color }
})

const canSubmit = computed(() => {
  if (props.mode === 'decrypt') {
    return password.value.length > 0
  }
  return password.value.length >= 8 && password.value === confirmPassword.value
})

watch(() => props.visible, (v) => {
  if (v) {
    password.value = ''
    confirmPassword.value = ''
    internalError.value = ''
    showPassword.value = false
  }
})

function handleSubmit() {
  if (!canSubmit.value) return
  
  if (props.mode === 'encrypt') {
    if (!/(?=.*[a-z])(?=.*[A-Z])(?=.*\d).{8,}/.test(password.value)) {
      internalError.value = '密码必须至少8字符，包含大小写字母和数字'
      return
    }
  }
  
  emit('submit', password.value)
}

function handleClose() {
  emit('close')
}
</script>

<template>
  <Teleport to="body">
    <Transition name="modal">
      <div v-if="visible" class="modal-overlay" @click.self="handleClose">
        <div class="modal-box password-modal">
          <button class="modal-close" @click="handleClose">
            <span class="i-carbon:close" />
          </button>

          <div class="modal-icon" :class="mode">
            <span v-if="mode === 'encrypt'" class="i-carbon:locked" />
            <span v-else class="i-carbon:locked" />
          </div>

          <h3 class="modal-title">
            {{ mode === 'encrypt' ? '设置密码' : '输入密码' }}
          </h3>

          <p class="modal-desc" v-if="mode === 'encrypt' && projectName">
            为「{{ projectName }}」设置导出密码
          </p>
          <p class="modal-desc" v-else-if="mode === 'decrypt'">
            请输入密码以解密文件
          </p>

          <div class="form-group">
            <label class="form-label">密码</label>
            <div class="input-wrapper">
              <input
                v-model="password"
                :type="showPassword ? 'text' : 'password'"
                class="form-input"
                :placeholder="mode === 'encrypt' ? '设置密码' : '输入密码'"
                @keyup.enter="canSubmit && handleSubmit()"
              />
              <button class="toggle-visibility" @click="showPassword = !showPassword">
                <span :class="showPassword ? 'i-carbon:view-off' : 'i-carbon:view'" />
              </button>
            </div>

            <div v-if="mode === 'encrypt'" class="strength-bar">
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

          <div v-if="mode === 'encrypt'" class="form-group">
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
            <span v-if="confirmPassword.length > 0 && password !== confirmPassword" class="field-error">
              两次输入的密码不一致
            </span>
          </div>

          <p v-if="error" class="submit-error">{{ error }}</p>

          <div class="modal-actions">
            <button class="btn btn-ghost" @click="handleClose">取消</button>
            <button
              class="btn btn-primary"
              :disabled="!canSubmit"
              @click="handleSubmit"
            >
              {{ mode === 'encrypt' ? '加密导出' : '解密并导入' }}
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
  border-radius: 12px;
  padding: 24px;
  min-width: 360px;
  max-width: 90vw;
  position: relative;
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

.password-modal {
  text-align: center;
}

.modal-icon {
  width: 48px;
  height: 48px;
  border-radius: 50%;
  display: flex;
  align-items: center;
  justify-content: center;
  margin: 0 auto 16px;
  font-size: 24px;
}

.modal-icon.encrypt {
  background: rgba(34, 197, 94, 0.15);
  color: #22c55e;
}

.modal-icon.decrypt {
  background: rgba(59, 130, 246, 0.15);
  color: #3b82f6;
}

.modal-title {
  font-size: 18px;
  font-weight: 600;
  margin: 0 0 8px;
  color: var(--studio-text);
}

.modal-desc {
  font-size: 14px;
  color: var(--studio-muted);
  margin: 0 0 20px;
}

.form-group {
  text-align: left;
  margin-bottom: 16px;
}

.form-label {
  display: block;
  font-size: 13px;
  font-weight: 500;
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
  border-radius: 8px;
  background: var(--studio-bg);
  color: var(--studio-text);
  font-size: 14px;
  outline: none;
  transition: border-color 0.15s;
}

.form-input:focus {
  border-color: var(--studio-accent);
}

.form-input.error {
  border-color: #ef4444;
}

.toggle-visibility {
  position: absolute;
  right: 8px;
  background: none;
  border: none;
  cursor: pointer;
  padding: 4px;
  color: var(--studio-muted);
}

.toggle-visibility:hover {
  color: var(--studio-text);
}

.strength-bar {
  display: flex;
  align-items: center;
  gap: 8px;
  margin-top: 8px;
}

.strength-track {
  flex: 1;
  height: 4px;
  background: var(--studio-border);
  border-radius: 2px;
  overflow: hidden;
}

.strength-fill {
  height: 100%;
  transition: width 0.3s, background 0.3s;
}

.strength-label {
  font-size: 12px;
  font-weight: 500;
}

.field-error {
  font-size: 12px;
  color: #ef4444;
  margin-top: 4px;
  display: block;
}

.submit-error {
  font-size: 13px;
  color: #ef4444;
  margin: 0 0 16px;
  padding: 8px 12px;
  background: rgba(239, 68, 68, 0.1);
  border-radius: 6px;
}

.modal-actions {
  display: flex;
  gap: 12px;
  justify-content: flex-end;
  margin-top: 20px;
}

.modal-actions .btn {
  min-width: 100px;
}
</style>
