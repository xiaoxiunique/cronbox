<script setup lang="ts">
import { ref } from "vue";
import { useRouter } from "vue-router";
import { loginWithToken, setAuthToken } from "../lib/api";

const router = useRouter();
const token = ref("");
const error = ref("");
const busy = ref(false);

async function submit() {
  const value = token.value.trim();
  if (!value || busy.value) return;
  error.value = "";
  busy.value = true;
  try {
    const ok = await loginWithToken(value);
    if (!ok) {
      error.value = "Invalid access token.";
      return;
    }
    setAuthToken(value);
    await router.replace("/");
  } catch {
    error.value = "Network error — is the CronBox server running?";
  } finally {
    busy.value = false;
  }
}
</script>

<template>
  <div class="login-page">
    <form class="login-card" @submit.prevent="submit">
      <div class="brand">CronBox</div>
      <p class="subtitle">Sign in to the control panel</p>
      <input
        v-model="token"
        type="password"
        placeholder="Access token"
        autocomplete="current-password"
        autofocus
      />
      <button type="submit" :disabled="busy || !token.trim()">Sign in</button>
      <p v-if="error" class="error">{{ error }}</p>
    </form>
  </div>
</template>

<style scoped>
.login-page {
  min-height: 100%;
  display: flex;
  align-items: center;
  justify-content: center;
}
.login-card {
  width: 340px;
  padding: 40px 36px;
  background: var(--bg-card);
  border: 1px solid var(--border);
  border-radius: var(--radius-lg);
  box-shadow: var(--card-shadow);
  backdrop-filter: var(--glass-blur);
}
.brand {
  font-size: 22px;
  font-weight: 700;
  letter-spacing: -0.02em;
}
.subtitle {
  margin: 4px 0 24px;
  color: var(--text-secondary);
  font-size: 13px;
}
input {
  width: 100%;
  padding: 10px 14px;
  margin-bottom: 14px;
  font-size: 14px;
  color: var(--text);
  background: var(--surface-solid);
  border: 1px solid var(--border-strong);
  border-radius: var(--radius-sm);
  outline: none;
}
input:focus {
  border-color: var(--accent);
  box-shadow: 0 0 0 3px var(--accent-strong);
}
button {
  width: 100%;
  padding: 10px 0;
  font-size: 14px;
  font-weight: 600;
  color: #fff;
  background: var(--accent);
  border: 0;
  border-radius: var(--radius-sm);
  cursor: pointer;
}
button:hover:not(:disabled) {
  background: var(--accent-600);
}
button:disabled {
  opacity: 0.5;
  cursor: default;
}
.error {
  margin: 12px 0 0;
  color: var(--danger);
  font-size: 13px;
}
</style>
