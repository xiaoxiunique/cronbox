<script setup lang="ts">
import { ref, onMounted, onUnmounted } from "vue";
import { api, type Job } from "../lib/api";
import { formatDateTime, formatDuration } from "../lib/format";

const jobs = ref<Job[]>([]);
const selected = ref<Job | null>(null);
const autoRefresh = ref(true);
let timer: number;

// Schedule dialog from job
const showScheduleDialog = ref(false);
const scheduleCron = ref("0 * * * *");
const scheduleTz = ref("Asia/Shanghai");
const scheduleArgs = ref("{}");
const scheduleScript = ref("");
const scheduleBaseDir = ref("");
const cronError = ref("");
const upcoming = ref<string[]>([]);

async function load() {
  jobs.value = await api.listJobs(100);
  if (selected.value) {
    try { selected.value = await api.getJob(selected.value.id); }
    catch { selected.value = null; }
  }
}

function statusIcon(s: string) {
  return { queued: "⏳", running: "🔄", success: "✅", failure: "❌", cancelled: "⛔", skipped: "↷" }[s] || "❓";
}
function statusClass(s: string) {
  return { success: "st-ok", failure: "st-err", running: "st-run", queued: "st-queue", cancelled: "st-cancel", skipped: "st-skip" }[s] || "";
}
function detailUrl(job: Job) {
  return { path: "/scripts/detail", query: { path: job.script_path, baseDir: job.base_dir } };
}
function formatArgs(args: string) {
  try { return JSON.stringify(JSON.parse(args), null, 2); }
  catch { return args; }
}

async function cleanup() {
  const n = await api.cleanupOldJobs(30);
  alert(`Cleaned ${n} jobs`);
  await load();
}

function copyArgs(job: Job) {
  navigator.clipboard.writeText(job.args);
}

async function rerun(job: Job) {
  try {
    await api.runNow(job.script_path, job.base_dir, job.args);
    await load();
  } catch (e: any) { alert(e); }
}

function openScheduleFromJob(job: Job) {
  scheduleScript.value = job.script_path;
  scheduleBaseDir.value = job.base_dir;
  scheduleArgs.value = job.args;
  scheduleCron.value = "0 * * * *";
  cronError.value = "";
  showScheduleDialog.value = true;
  checkCron();
}

async function checkCron() {
  try {
    await api.validateCron(scheduleCron.value);
    cronError.value = "";
    upcoming.value = await api.upcomingRuns(scheduleCron.value, scheduleTz.value);
  } catch (e: any) { cronError.value = e; upcoming.value = []; }
}

async function saveSchedule() {
  try {
    await api.createSchedule(scheduleScript.value, scheduleBaseDir.value, scheduleCron.value, scheduleTz.value, scheduleArgs.value);
    showScheduleDialog.value = false;
    alert("Schedule created!");
  } catch (e: any) { alert(e); }
}

onMounted(() => { load(); timer = setInterval(() => { if (autoRefresh.value) load(); }, 3000); });
onUnmounted(() => clearInterval(timer));
</script>

<template>
  <div class="history">
    <div class="header">
      <h2>History</h2>
      <label class="auto"><input type="checkbox" v-model="autoRefresh" /> Auto</label>
      <button @click="load" class="btn-icon">↻</button>
      <button @click="cleanup" class="btn sm">Cleanup 30d</button>
    </div>

    <div v-if="jobs.length === 0" class="empty">No jobs yet.</div>

    <div v-else class="split">
      <div class="job-list">
        <div v-for="j in jobs" :key="j.id"
          :class="['job-row', { active: selected?.id === j.id }]"
          @click="selected = j"
        >
          <span class="icon">{{ statusIcon(j.status) }}</span>
          <div class="job-info">
            <router-link :to="detailUrl(j)" class="job-name job-link" @click.stop>
              {{ j.script_path }}
            </router-link>
            <div class="job-meta">
              {{ formatDateTime(j.created_at) }}
              <span v-if="j.duration_ms != null"> · {{ formatDuration(j.duration_ms) }}</span>
            </div>
          </div>
          <span :class="['badge', statusClass(j.status)]">{{ j.status }}</span>
        </div>
      </div>

      <div class="detail" v-if="selected">
        <div class="detail-header">
          <router-link :to="detailUrl(selected)" class="detail-title">{{ selected.script_path }}</router-link>
          <div class="detail-actions">
            <button @click="rerun(selected)" class="btn sm" title="Re-run with same args">▶ Re-run</button>
            <button @click="openScheduleFromJob(selected)" class="btn sm" title="Create schedule from this run">⏰ Schedule</button>
          </div>
        </div>

        <table class="meta-table"><tbody>
          <tr><td>ID</td><td class="mono">{{ selected.id }}</td></tr>
          <tr><td>Status</td><td><span :class="['badge', statusClass(selected.status)]">{{ selected.status }}</span></td></tr>
          <tr v-if="selected.started_at"><td>Started</td><td>{{ formatDateTime(selected.started_at) }}</td></tr>
          <tr v-if="selected.completed_at"><td>Completed</td><td>{{ formatDateTime(selected.completed_at) }}</td></tr>
          <tr v-if="selected.duration_ms != null"><td>Duration</td><td>{{ formatDuration(selected.duration_ms) }}</td></tr>
        </tbody></table>

        <!-- Args section with copy button -->
        <div v-if="selected.args && selected.args !== '{}'" class="section">
          <div class="section-title">
            Arguments
            <button @click="copyArgs(selected)" class="copy-btn" title="Copy args JSON">📋</button>
          </div>
          <pre class="args-box">{{ formatArgs(selected.args) }}</pre>
        </div>

        <div v-if="selected.error" class="section">
          <div class="section-title">Error</div>
          <pre class="error-box">{{ selected.error }}</pre>
        </div>
        <div class="section">
          <div class="section-title">Logs</div>
          <pre class="log-box">{{ selected.logs || '(no output)' }}</pre>
        </div>
        <div v-if="selected.result" class="section">
          <div class="section-title">Result</div>
          <pre class="log-box">{{ selected.result }}</pre>
        </div>
      </div>
      <div v-else class="detail empty-detail">Select a job to view details</div>
    </div>

    <!-- Schedule from Job dialog -->
    <div v-if="showScheduleDialog" class="overlay" @click.self="showScheduleDialog = false">
      <div class="dialog">
        <h3>Schedule: {{ scheduleScript }}</h3>

        <div class="dialog-info">
          <span class="dialog-info-label">Args from job:</span>
          <pre class="dialog-args-preview">{{ formatArgs(scheduleArgs) }}</pre>
        </div>

        <label>Cron Expression</label>
        <input v-model="scheduleCron" @input="checkCron" class="mono" placeholder="0 * * * *" />
        <div v-if="cronError" class="error">{{ cronError }}</div>

        <label>Timezone</label>
        <input v-model="scheduleTz" />

        <label>Arguments (JSON)</label>
        <textarea v-model="scheduleArgs" class="mono" rows="4"></textarea>

        <div v-if="upcoming.length" class="upcoming">
          <div class="upcoming-title">Next runs:</div>
          <div v-for="t in upcoming" :key="t" class="upcoming-time">{{ t }}</div>
        </div>

        <div class="dialog-actions">
          <button @click="showScheduleDialog = false" class="btn">Cancel</button>
          <button @click="saveSchedule" class="btn primary" :disabled="!!cronError">Create Schedule</button>
        </div>
      </div>
    </div>
  </div>
</template>

<style scoped>
.history { display: flex; flex-direction: column; height: calc(100vh - 144px); }
.header { display: flex; align-items: center; gap: 8px; margin-bottom: 16px; }
.header h2 { flex: 1; font-size: 32px; font-weight: 760; letter-spacing: -0.04em; margin: 0; }
.auto { font-size: 12px; color: var(--text-secondary); display: flex; align-items: center; gap: 4px; }
.empty {
  text-align: center;
  padding: 48px;
  color: var(--text-secondary);
  background: var(--bg-card);
  border: 1px solid var(--border);
  border-radius: var(--radius);
  backdrop-filter: var(--glass-blur);
  -webkit-backdrop-filter: var(--glass-blur);
  box-shadow: var(--glass-shadow-soft), var(--glass-highlight);
}

.split { display: flex; flex: 1; gap: 12px; min-height: 0; }
.job-list {
  width: 360px;
  overflow-y: auto;
  display: flex;
  flex-direction: column;
  gap: 4px;
  padding: 8px;
}
.job-row {
  display: flex; align-items: center; gap: 8px; padding: 8px 10px;
  border: 1px solid transparent;
  border-radius: var(--radius-sm);
  cursor: pointer;
  transition: background 0.16s ease, border-color 0.16s ease, transform 0.16s ease, box-shadow 0.16s ease;
}
.job-row:hover { background: var(--bg-soft); border-color: var(--border); }
.job-row.active {
  background:
    linear-gradient(135deg, var(--accent-soft), transparent 60%),
    var(--bg-elevated);
  border-color: var(--accent-strong);
  box-shadow: var(--glass-shadow-soft);
}
.job-row.active .job-meta { color: var(--text-secondary); }
.icon { font-size: 16px; }
.job-info { flex: 1; min-width: 0; }
.job-name { font-size: 13px; font-weight: 500; white-space: nowrap; overflow: hidden; text-overflow: ellipsis; }
.job-link,
.detail-title {
  display: block;
  color: inherit;
  text-decoration: none;
  border-radius: 7px;
}
.job-link:hover,
.detail-title:hover { color: var(--accent); }
.detail-title { font-size: 16px; font-weight: 650; min-width: 0; overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
.job-meta { font-size: 11px; color: var(--text-secondary); }

.detail { flex: 1; overflow-y: auto; padding: 18px; }
.detail-header { display: flex; align-items: center; justify-content: space-between; gap: 12px; margin-bottom: 12px; }
.detail-actions { display: flex; gap: 6px; }
.empty-detail { display: flex; align-items: center; justify-content: center; color: var(--text-secondary); }
.meta-table { font-size: 13px; margin-bottom: 12px; }
.meta-table td { padding: 3px 12px 3px 0; }
.meta-table td:first-child { color: var(--text-secondary); white-space: nowrap; }
.mono { font-family: 'SF Mono', monospace; font-size: 11px; }
.section { margin-top: 12px; }
.section-title { font-size: 12px; font-weight: 600; color: var(--text-secondary); margin-bottom: 4px; display: flex; align-items: center; gap: 6px; }
.copy-btn { border: none; background: none; cursor: pointer; font-size: 13px; padding: 0; opacity: 0.6; }
.copy-btn:hover { opacity: 1; }
.log-box, .error-box, .args-box {
  background: var(--bg-code);
  border: 1px solid rgba(255, 255, 255, 0.08);
  padding: 12px;
  border-radius: var(--radius-sm);
  color: #e6e6ea;
  font-size: 12px;
  font-family: 'SF Mono', monospace; white-space: pre-wrap; word-break: break-all;
  max-height: 300px; overflow-y: auto;
}
.error-box { color: #ff8a80; }
.args-box { max-height: 120px; }

/* Dialog */
.overlay {
  position: fixed;
  inset: 0;
  background: rgba(10,14,20,0.28);
  backdrop-filter: blur(10px);
  -webkit-backdrop-filter: blur(10px);
  display: flex;
  align-items: center;
  justify-content: center;
  z-index: 100;
}
.dialog {
  padding: 24px;
  width: 460px;
  max-height: 80vh;
  overflow-y: auto;
}
.dialog h3 { margin: 0 0 16px; font-size: 16px; }
.dialog label { display: block; font-size: 12px; color: var(--text-secondary); margin: 12px 0 4px; }
.dialog input, .dialog textarea {
  width: 100%; padding: 8px 10px; border: 1px solid var(--border); border-radius: var(--radius-sm);
  background: var(--surface-subtle); color: var(--text); font-size: 13px;
}
.dialog input:focus, .dialog textarea:focus {
  outline: none;
  border-color: var(--accent-strong);
  box-shadow: 0 0 0 3px var(--accent-soft);
}
.dialog-info { margin-bottom: 12px; }
.dialog-info-label { font-size: 11px; color: var(--text-secondary); }
.dialog-args-preview {
  background: var(--bg-code); border: 1px solid rgba(255, 255, 255, 0.08); padding: 10px; border-radius: var(--radius-sm); font-size: 11px;
  color: #e6e6ea;
  font-family: 'SF Mono', monospace; max-height: 80px; overflow-y: auto; margin-top: 4px;
}
.error { color: var(--danger); font-size: 12px; margin-top: 4px; }
.upcoming { margin-top: 12px; background: var(--bg-soft); border: 1px solid var(--border); border-radius: var(--radius-sm); padding: 8px 10px; }
.upcoming-title { font-size: 12px; color: var(--text-secondary); margin-bottom: 4px; }
.upcoming-time { font-size: 11px; font-family: monospace; color: var(--text-secondary); }
.dialog-actions { display: flex; justify-content: flex-end; gap: 8px; margin-top: 20px; }
</style>
