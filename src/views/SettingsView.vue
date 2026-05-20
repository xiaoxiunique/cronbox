<script setup lang="ts">
import { computed, ref, onMounted } from "vue";
import { api, type ScriptFile, type WorkDir } from "../lib/api";
import { open } from "@tauri-apps/plugin-dialog";
import LanguageLogo from "../components/LanguageLogo.vue";

const workDirs = ref<WorkDir[]>([]);
const cliStatus = ref("");
const resolvedPath = ref("");
const showPicker = ref(false);
const pickerBaseDir = ref("");
const pickerScripts = ref<ScriptFile[]>([]);
const selectedPaths = ref<Set<string>>(new Set());
const savingPicker = ref(false);

const selectedCount = computed(() => selectedPaths.value.size);
const resolvedPathDirs = computed(() => resolvedPath.value.split(":").filter(Boolean));

async function load() {
  workDirs.value = await api.listWorkDirs();
  cliStatus.value = await api.cliStatus();
  resolvedPath.value = await api.resolvedPath();
}

async function addDir() {
  try {
    const selected = await open({ directory: true, multiple: false });
    if (selected) {
      const path = selected as string;
      const scripts = await api.previewScriptEntries(path);
      if (scripts.length === 0) {
        alert("No executable entry scripts were found.");
        return;
      }
      pickerBaseDir.value = path;
      pickerScripts.value = scripts;
      selectedPaths.value = new Set();
      showPicker.value = true;
    }
  } catch (e: any) {
    console.error("addDir error:", e);
    alert("Failed: " + e);
  }
}

async function addFile() {
  try {
    const selected = await open({
      directory: false,
      multiple: false,
      filters: [{ name: "Scripts", extensions: ["py", "sh", "bash", "ts", "js", "mts", "mjs", "sql"] }],
    });
    if (!selected) return;
    const added = await api.addScriptFile(selected as string);
    await load();
    if (added.entry_scripts.length === 0) {
      alert("No script was added.");
    }
  } catch (e: any) {
    console.error("addFile error:", e);
    alert("Failed: " + e);
  }
}

function toggleScript(path: string, checked: boolean) {
  const next = new Set(selectedPaths.value);
  if (checked) {
    next.add(path);
  } else {
    next.delete(path);
  }
  selectedPaths.value = next;
}

function selectAllScripts() {
  selectedPaths.value = new Set(pickerScripts.value.map((script) => script.path));
}

function clearSelectedScripts() {
  selectedPaths.value = new Set();
}

async function saveSelectedScripts() {
  if (selectedPaths.value.size === 0) {
    alert("Select at least one script.");
    return;
  }
  savingPicker.value = true;
  try {
    await api.addSelectedScripts(pickerBaseDir.value, Array.from(selectedPaths.value));
    showPicker.value = false;
    await load();
  } catch (e: any) {
    alert("Failed: " + e);
  } finally {
    savingPicker.value = false;
  }
}

async function removeDir(id: string) {
  await api.removeWorkDir(id);
  await load();
}

async function cleanup(days: number) {
  const n = await api.cleanupOldJobs(days);
  alert(`Cleaned ${n} old jobs`);
}

async function installCli() {
  try {
    const path = await api.installCli(true);
    cliStatus.value = await api.cliStatus();
    alert(`Installed cronbox command at ${path}`);
  } catch (e: any) {
    alert("Failed: " + e);
  }
}

onMounted(load);
</script>

<template>
  <div class="settings-page">
    <div class="header"><h2>Settings</h2></div>

    <div class="section">
      <div class="section-title">Working Directories</div>
      <div class="card">
        <div v-if="workDirs.length === 0" class="empty-dirs">No directories added yet</div>
        <div v-for="wd in workDirs" :key="wd.id" class="dir-row">
          <span class="dir-icon">{{ wd.scan_mode === "manual" ? "☑" : "📁" }}</span>
          <span class="dir-path">{{ wd.path }}</span>
          <span class="dir-mode">{{ wd.scan_mode }}</span>
          <button @click="removeDir(wd.id)" class="btn-icon del" title="Remove">✕</button>
        </div>
        <div class="source-actions">
          <button @click="addFile" class="btn add-btn">+ Add Script</button>
          <button @click="addDir" class="btn">+ Add Directory</button>
        </div>
        <div class="hint">Add a single script file, or scan a directory and choose which entry scripts to include.</div>
      </div>
    </div>

    <div class="section">
      <div class="section-title">Maintenance</div>
      <div class="card">
        <button @click="cleanup(30)" class="btn">Cleanup jobs older than 30 days</button>
        <button @click="cleanup(7)" class="btn">Cleanup jobs older than 7 days</button>
      </div>
    </div>

    <div class="section">
      <div class="section-title">Command Line</div>
      <div class="card">
        <div class="cli-status">{{ cliStatus }}</div>
        <button @click="installCli" class="btn add-btn">Install cronbox command</button>
        <div class="hint">After installation, use cronbox help in Terminal to manage directories, schedules, jobs, and manual runs.</div>
      </div>
    </div>

    <div class="section">
      <div class="section-title">Script Environment</div>
      <div class="card">
        <div class="hint">
          Scripts run with this PATH, resolved from your login shell at startup. This is what lets
          scheduled scripts find tools like bun, uv, and Homebrew binaries.
        </div>
        <div class="path-list">
          <div v-for="dir in resolvedPathDirs" :key="dir" class="path-dir">{{ dir }}</div>
          <div v-if="resolvedPathDirs.length === 0" class="empty-dirs">PATH not resolved yet</div>
        </div>
      </div>
    </div>

    <div class="section">
      <div class="section-title">About</div>
      <div class="card">
        <div>Version: 0.1.0</div>
        <div>Engine: cronbox (Rust + SQLite + Tauri 2)</div>
      </div>
    </div>

    <div v-if="showPicker" class="overlay" @click.self="showPicker = false">
      <div class="dialog picker-dialog">
        <div class="dialog-head">
          <div>
            <h3>Select Scripts</h3>
            <div class="picker-base">{{ pickerBaseDir }}</div>
          </div>
          <button @click="showPicker = false" class="btn-icon" title="Close">✕</button>
        </div>

        <div class="picker-toolbar">
          <span>{{ selectedCount }} of {{ pickerScripts.length }} selected</span>
          <div>
            <button @click="selectAllScripts" class="link-btn">Select all</button>
            <button @click="clearSelectedScripts" class="link-btn">Clear</button>
          </div>
        </div>
        <div class="picker-note">
          Detection is heuristic — a shebang or a Python <span class="mono">__main__</span> block
          does not always mean a file is a standalone task. Check the matched reason and pick only
          the scripts you actually want to run.
        </div>

        <div class="script-picker-list">
          <label v-for="script in pickerScripts" :key="script.path" class="script-pick-row">
            <input
              type="checkbox"
              :checked="selectedPaths.has(script.path)"
              @change="toggleScript(script.path, ($event.target as HTMLInputElement).checked)"
            />
            <LanguageLogo :language="script.language" />
            <span class="pick-text">
              <span class="pick-name">{{ script.alias }}</span>
              <span class="pick-path">{{ script.path }}</span>
              <span class="pick-reason">matched: {{ script.entry_reason }}</span>
            </span>
          </label>
        </div>

        <div class="dialog-actions">
          <button @click="showPicker = false" class="btn">Cancel</button>
          <button @click="saveSelectedScripts" class="btn add-btn" :disabled="savingPicker || selectedCount === 0">
            Add Selected
          </button>
        </div>
      </div>
    </div>
  </div>
</template>

<style scoped>
.settings-page { min-width: 0; }
.header { margin-bottom: 20px; }
.header h2 { font-size: 20px; margin: 0; }
.section { margin-bottom: 20px; }
.section-title { font-size: 12px; font-weight: 650; color: var(--text-secondary); margin-bottom: 7px; text-transform: uppercase; letter-spacing: 0.5px; }
.card {
  background:
    linear-gradient(180deg, rgba(255, 255, 255, 0.34), rgba(255, 255, 255, 0.10)),
    var(--bg-card);
  border: 1px solid var(--border);
  border-radius: 8px;
  padding: 14px;
  display: flex;
  flex-direction: column;
  gap: 8px;
  backdrop-filter: var(--glass-blur);
  -webkit-backdrop-filter: var(--glass-blur);
  box-shadow: var(--glass-shadow-soft), var(--glass-highlight);
}
.empty-dirs { color: var(--text-secondary); font-size: 13px; text-align: center; padding: 8px; }
.dir-row {
  display: flex;
  align-items: center;
  gap: 8px;
  padding: 7px 8px;
  background: var(--bg-soft);
  border: 1px solid var(--border);
  border-radius: 8px;
}
.dir-icon { font-size: 16px; }
.dir-path { flex: 1; font-family: monospace; font-size: 13px; overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
.dir-mode {
  color: var(--text-secondary);
  font-size: 11px;
  border: 1px solid var(--border);
  border-radius: 999px;
  padding: 2px 7px;
  background: var(--bg-soft);
}
.source-actions {
  display: flex;
  gap: 8px;
  flex-wrap: wrap;
}
.btn-icon {
  border: 1px solid var(--border);
  background: var(--bg-soft);
  cursor: pointer;
  font-size: 14px;
  color: var(--text-secondary);
  border-radius: 8px;
  min-width: 28px;
  height: 28px;
}
.btn-icon:hover { background: var(--bg-elevated); border-color: var(--border-strong); color: var(--text); }
.btn-icon.del:hover { color: var(--danger); }
.add-btn { align-self: flex-start; }
.hint { font-size: 11px; color: var(--text-secondary); }
.cli-status {
  font-family: monospace;
  font-size: 12px;
  color: var(--text-secondary);
  overflow-wrap: anywhere;
  background: var(--bg-code);
  border: 1px solid var(--border);
  border-radius: 8px;
  padding: 8px 10px;
}
.path-list {
  display: flex;
  flex-direction: column;
  gap: 2px;
  background: var(--bg-code);
  border: 1px solid var(--border);
  border-radius: 8px;
  padding: 8px 10px;
  max-height: 220px;
  overflow-y: auto;
}
.path-dir {
  font-family: monospace;
  font-size: 12px;
  color: var(--text-secondary);
  overflow-wrap: anywhere;
}
.btn {
  padding: 6px 14px;
  border: 1px solid var(--border);
  border-radius: 8px;
  background: var(--bg-soft);
  cursor: pointer;
  color: var(--text);
  font-size: 13px;
  box-shadow: var(--glass-highlight);
}
.btn:not(.add-btn):hover { background: var(--bg-elevated); border-color: var(--border-strong); }
.add-btn {
  background: linear-gradient(135deg, var(--accent), #49a4ff);
  color: #fff;
  border-color: transparent;
  box-shadow: 0 10px 26px rgba(22, 119, 255, 0.22);
}
.add-btn:hover {
  background: linear-gradient(135deg, #0f6fe8, #3d98f5);
  color: #fff;
  border-color: transparent;
  box-shadow: 0 12px 30px rgba(22, 119, 255, 0.30);
}
.overlay {
  position: fixed;
  inset: 0;
  background: rgba(10, 14, 20, 0.28);
  backdrop-filter: blur(10px);
  -webkit-backdrop-filter: blur(10px);
  display: flex;
  align-items: center;
  justify-content: center;
  z-index: 100;
}
.dialog {
  width: min(640px, calc(100vw - 40px));
  max-height: 82vh;
  overflow: hidden;
  background: var(--bg-elevated);
  border: 1px solid var(--border-strong);
  border-radius: 12px;
  padding: 20px;
  box-shadow: var(--glass-shadow), var(--glass-highlight);
  backdrop-filter: var(--glass-blur);
  -webkit-backdrop-filter: var(--glass-blur);
  display: flex;
  flex-direction: column;
}
.dialog-head {
  display: flex;
  justify-content: space-between;
  gap: 12px;
  align-items: flex-start;
  margin-bottom: 14px;
}
.dialog h3 { margin: 0; font-size: 16px; }
.picker-base {
  margin-top: 4px;
  color: var(--text-secondary);
  font-family: monospace;
  font-size: 12px;
  overflow-wrap: anywhere;
}
.picker-toolbar {
  display: flex;
  justify-content: space-between;
  align-items: center;
  gap: 12px;
  color: var(--text-secondary);
  font-size: 12px;
  margin-bottom: 10px;
}
.link-btn {
  border: none;
  background: transparent;
  color: var(--accent);
  cursor: pointer;
  font-size: 12px;
  padding: 3px 6px;
}
.link-btn:hover { color: var(--text); }
.script-picker-list {
  overflow-y: auto;
  min-height: 120px;
  display: flex;
  flex-direction: column;
  gap: 8px;
  padding-right: 4px;
}
.script-pick-row {
  display: grid;
  grid-template-columns: 18px 30px minmax(0, 1fr);
  align-items: center;
  gap: 10px;
  background: var(--bg-soft);
  border: 1px solid var(--border);
  border-radius: 8px;
  padding: 9px 10px;
  cursor: pointer;
}
.script-pick-row:hover {
  background: var(--bg-elevated);
  border-color: var(--border-strong);
}
.script-pick-row input {
  width: 16px;
  height: 16px;
  accent-color: var(--accent);
}
.pick-text { min-width: 0; }
.pick-name {
  display: block;
  font-size: 13px;
  font-weight: 650;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}
.pick-path {
  display: block;
  margin-top: 2px;
  color: var(--text-secondary);
  font-family: monospace;
  font-size: 11px;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}
.pick-reason {
  display: block;
  margin-top: 3px;
  font-size: 10px;
  font-family: monospace;
  color: var(--warning);
}
.picker-note {
  font-size: 11px;
  line-height: 1.5;
  color: var(--text-secondary);
  background: rgba(255, 149, 0, 0.1);
  border: 1px solid rgba(255, 149, 0, 0.22);
  border-radius: 8px;
  padding: 7px 9px;
  margin-bottom: 10px;
}
.mono { font-family: monospace; }
.dialog-actions {
  display: flex;
  justify-content: flex-end;
  gap: 8px;
  margin-top: 16px;
}
@media (prefers-color-scheme: dark) {
  .card {
    background:
      linear-gradient(180deg, rgba(255, 255, 255, 0.08), rgba(255, 255, 255, 0.02)),
      var(--bg-card);
  }
}
</style>
