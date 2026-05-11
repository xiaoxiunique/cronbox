<script setup lang="ts">
import { ref, onMounted } from "vue";
import { api, type WorkDir } from "../lib/api";
import { open } from "@tauri-apps/plugin-dialog";

const workDirs = ref<WorkDir[]>([]);
const cliStatus = ref("");

async function load() {
  workDirs.value = await api.listWorkDirs();
  cliStatus.value = await api.cliStatus();
}

async function addDir() {
  try {
    const selected = await open({ directory: true, multiple: false });
    if (selected) {
      const added = await api.addWorkDirWithScan(selected as string);
      await load();
      if (added.entry_scripts.length === 0) {
        alert("Directory added, but no executable entry scripts were found.");
      }
    }
  } catch (e: any) {
    console.error("addDir error:", e);
    alert("Failed: " + e);
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
          <span class="dir-icon">📁</span>
          <span class="dir-path">{{ wd.path }}</span>
          <button @click="removeDir(wd.id)" class="btn-icon del" title="Remove">✕</button>
        </div>
        <button @click="addDir" class="btn add-btn">+ Add Directory</button>
        <div class="hint">CronBox scans these directories for .py .sh .ts .js .sql files</div>
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
      <div class="section-title">About</div>
      <div class="card">
        <div>Version: 0.1.0</div>
        <div>Engine: cronbox (Rust + SQLite + Tauri 2)</div>
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
@media (prefers-color-scheme: dark) {
  .card {
    background:
      linear-gradient(180deg, rgba(255, 255, 255, 0.08), rgba(255, 255, 255, 0.02)),
      var(--bg-card);
  }
}
</style>
