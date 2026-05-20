<script setup lang="ts">
interface EnvPair {
  key: string;
  value: string;
}

const props = defineProps<{ modelValue: EnvPair[]; disabled?: boolean }>();
const emit = defineEmits<{ "update:modelValue": [EnvPair[]] }>();

function setKey(index: number, key: string) {
  const next = props.modelValue.map((pair, i) => (i === index ? { ...pair, key } : pair));
  emit("update:modelValue", next);
}

function setValue(index: number, value: string) {
  const next = props.modelValue.map((pair, i) => (i === index ? { ...pair, value } : pair));
  emit("update:modelValue", next);
}

function removePair(index: number) {
  emit("update:modelValue", props.modelValue.filter((_, i) => i !== index));
}

function addPair() {
  emit("update:modelValue", [...props.modelValue, { key: "", value: "" }]);
}
</script>

<template>
  <div class="env-editor">
    <div v-for="(pair, i) in modelValue" :key="i" class="env-row">
      <input
        class="env-key mono"
        :value="pair.key"
        placeholder="KEY"
        :disabled="disabled"
        @input="setKey(i, ($event.target as HTMLInputElement).value)"
      />
      <input
        class="env-val mono"
        :value="pair.value"
        placeholder="value"
        :disabled="disabled"
        @input="setValue(i, ($event.target as HTMLInputElement).value)"
      />
      <button type="button" class="env-del" title="Remove" :disabled="disabled" @click="removePair(i)">✕</button>
    </div>
    <button type="button" class="env-add" :disabled="disabled" @click="addPair">+ Add variable</button>
  </div>
</template>

<style scoped>
.env-editor { display: flex; flex-direction: column; gap: 6px; }
.env-row {
  display: grid;
  grid-template-columns: minmax(0, 1fr) minmax(0, 1.4fr) 30px;
  gap: 6px;
  align-items: center;
}
.env-row input {
  width: 100%;
  padding: 6px 8px;
  border: 1px solid var(--border);
  border-radius: 8px;
  background: var(--bg-soft);
  color: var(--text);
  font-size: 12px;
}
.env-row input:focus {
  outline: none;
  border-color: var(--accent-strong);
  box-shadow: 0 0 0 3px var(--accent-soft);
}
.env-del {
  border: 1px solid var(--border);
  background: var(--bg-soft);
  color: var(--text-secondary);
  border-radius: 8px;
  height: 30px;
  cursor: pointer;
  font-size: 12px;
}
.env-del:hover { color: var(--danger); border-color: var(--border-strong); }
.env-add {
  align-self: flex-start;
  border: 1px dashed var(--border);
  background: transparent;
  color: var(--text-secondary);
  border-radius: 8px;
  padding: 5px 10px;
  cursor: pointer;
  font-size: 12px;
}
.env-add:hover { color: var(--accent); border-color: var(--accent-strong); }
.env-add:disabled, .env-del:disabled, .env-row input:disabled { opacity: 0.5; cursor: default; }
.mono { font-family: monospace; }
</style>
