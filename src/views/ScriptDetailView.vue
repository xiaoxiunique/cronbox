<script setup lang="ts">
import { computed, onMounted, onUnmounted, ref, watch } from "vue";
import { useRoute, useRouter } from "vue-router";
import LanguageLogo from "../components/LanguageLogo.vue";
import { api, type Job, type Schedule, type ScriptFile } from "../lib/api";
import { formatDateTime, formatDuration } from "../lib/format";

const route = useRoute();
const router = useRouter();

const scripts = ref<ScriptFile[]>([]);
const schedules = ref<Schedule[]>([]);
const jobs = ref<Job[]>([]);
const selected = ref<Job | null>(null);
const loading = ref(false);
const error = ref("");
let timer: number | null = null;

const scriptPath = computed(() => String(route.query.path ?? ""));
const baseDir = computed(() => String(route.query.baseDir ?? ""));
const hasScriptTarget = computed(() => Boolean(scriptPath.value && baseDir.value));

const script = computed(() =>
  scripts.value.find((s) => s.path === scriptPath.value && s.base_dir === baseDir.value) ?? null
);

const schedule = computed(() =>
  schedules.value.find((s) => s.script_path === scriptPath.value && s.base_dir === baseDir.value) ?? null
);

const displayName = computed(() => script.value?.alias ?? script.value?.name ?? scriptPath.value.split("/").pop() ?? "Script");
const language = computed(() => script.value?.language ?? "file");
const fullPath = computed(() =>
  baseDir.value && scriptPath.value ? `${baseDir.value}/${scriptPath.value}` : scriptPath.value
);

const stats = computed(() => {
  const counts = {
    total: jobs.value.length,
    success: 0,
    failure: 0,
    running: 0,
    skipped: 0,
  };
  for (const job of jobs.value) {
    if (job.status === "success") counts.success += 1;
    if (job.status === "failure") counts.failure += 1;
    if (job.status === "running" || job.status === "queued") counts.running += 1;
    if (job.status === "skipped") counts.skipped += 1;
  }
  return counts;
});

const selectedLogContent = computed(() => {
  const job = selected.value;
  if (!job) return "Select a run from the left to inspect logs.";

  const lines: string[] = [];
  const source = job.schedule_id ? "schedule" : "manual";

  lines.push(`[cronbox] job ${job.id}`);
  lines.push(`script: ${job.base_dir}/${job.script_path}`);
  lines.push(`status: ${job.status}`);
  lines.push(`source: ${source}`);
  lines.push(`created: ${formatDateTime(job.created_at)}`);
  if (job.scheduled_for) lines.push(`scheduled_for: ${formatDateTime(job.scheduled_for)}`);
  if (job.started_at) lines.push(`started: ${formatDateTime(job.started_at)}`);
  if (job.completed_at) lines.push(`completed: ${formatDateTime(job.completed_at)}`);
  if (job.duration_ms != null) lines.push(`duration: ${formatDuration(job.duration_ms)}`);

  if (schedule.value) {
    lines.push("");
    lines.push("[schedule]");
    lines.push(`cron: ${schedule.value.cron_expr}`);
    lines.push(`timezone: ${schedule.value.timezone}`);
    lines.push(`enabled: ${schedule.value.enabled}`);
    if (schedule.value.next_run_at) lines.push(`next_run: ${shortTime(schedule.value.next_run_at)}`);
  }

  lines.push("");
  lines.push("[inputs]");
  lines.push(formatArgs(job.args || "{}"));

  lines.push("");
  lines.push("[logs]");
  if (job.logs) {
    lines.push(job.logs);
  } else if (job.status === "queued") {
    lines.push("Waiting to start...");
  } else if (job.status === "running") {
    lines.push("Running...");
  } else {
    lines.push("(no output)");
  }

  if (job.error) {
    lines.push("");
    lines.push("[error]");
    lines.push(job.error);
  }

  if (job.result) {
    lines.push("");
    lines.push("[result]");
    lines.push(job.result);
  }

  return lines.join("\n");
});

async function load() {
  if (!hasScriptTarget.value) {
    error.value = "Missing script path.";
    scripts.value = [];
    schedules.value = [];
    jobs.value = [];
    selected.value = null;
    return;
  }

  loading.value = true;
  error.value = "";
  try {
    const [nextScripts, nextSchedules, nextJobs] = await Promise.all([
      api.scanScripts(),
      api.listSchedules(),
      api.listJobsForScript(scriptPath.value, baseDir.value, 200),
    ]);
    scripts.value = nextScripts;
    schedules.value = nextSchedules;
    jobs.value = nextJobs;
    if (selected.value) {
      selected.value = nextJobs.find((j) => j.id === selected.value?.id) ?? nextJobs[0] ?? null;
    } else {
      selected.value = nextJobs[0] ?? null;
    }
  } catch (e: any) {
    error.value = String(e);
  } finally {
    loading.value = false;
  }
}

async function runNow() {
  if (!hasScriptTarget.value) return;
  try {
    const job = await api.runNow(scriptPath.value, baseDir.value);
    await load();
    selected.value = jobs.value.find((j) => j.id === job.id) ?? job;
  } catch (e: any) {
    alert(e);
  }
}

async function rerun(job: Job) {
  try {
    const nextJob = await api.runNow(job.script_path, job.base_dir, job.args);
    await load();
    selected.value = jobs.value.find((j) => j.id === nextJob.id) ?? nextJob;
  } catch (e: any) {
    alert(e);
  }
}

async function copyLog() {
  await navigator.clipboard.writeText(selectedLogContent.value);
}

function statusIcon(status: string) {
  return {
    queued: "⏳",
    running: "🔄",
    success: "✅",
    failure: "❌",
    cancelled: "⛔",
    skipped: "↷",
  }[status] || "❓";
}

function statusClass(status: string) {
  return {
    success: "st-ok",
    failure: "st-err",
    running: "st-run",
    queued: "st-queue",
    cancelled: "st-cancel",
    skipped: "st-skip",
  }[status] || "";
}

function shortTime(iso: string | null) {
  if (!iso) return "";
  const diff = new Date(iso).getTime() - Date.now();
  const mins = Math.round(diff / 60000);
  if (Math.abs(mins) < 1) return "< 1min";
  if (Math.abs(mins) < 60) return mins > 0 ? `in ${mins}m` : `${-mins}m ago`;
  const hours = Math.round(mins / 60);
  return hours > 0 ? `in ${hours}h` : `${-hours}h ago`;
}

function formatArgs(args: string) {
  try {
    return JSON.stringify(JSON.parse(args), null, 2);
  } catch {
    return args;
  }
}

watch(
  () => [route.query.path, route.query.baseDir],
  () => {
    selected.value = null;
    load();
  },
  { immediate: true }
);

onMounted(() => {
  timer = window.setInterval(() => {
    if (jobs.value.some((j) => j.status === "queued" || j.status === "running")) {
      load();
    }
  }, 1500);
});

onUnmounted(() => {
  if (timer !== null) clearInterval(timer);
});
</script>

<template>
  <div class="detail-page">
    <div class="topbar">
      <button class="btn-icon" title="Back" @click="router.push('/scripts')">←</button>
      <LanguageLogo :language="language" />
      <div class="title-wrap">
        <h2>{{ displayName }}</h2>
        <div class="subtitle">{{ fullPath }}</div>
      </div>
      <button class="btn" @click="load" :disabled="loading">Refresh</button>
      <button class="btn primary" @click="runNow" :disabled="!hasScriptTarget || loading">Run</button>
    </div>

    <div v-if="error" class="empty">{{ error }}</div>

    <template v-else>
      <section class="history-shell">
        <div class="history-panel">
          <div class="panel-head">
            <div>
              <div class="panel-title">Runs</div>
              <div class="panel-subtitle">
                {{ stats.total }} total · {{ stats.success }} ok · {{ stats.failure }} failed · {{ stats.skipped }} skipped
              </div>
            </div>
            <span class="count-chip">{{ language }}</span>
          </div>
          <div class="job-list">
            <div v-if="loading && jobs.length === 0" class="empty-inline">Loading runs...</div>
            <div v-else-if="jobs.length === 0" class="empty-inline">No runs yet.</div>
            <button
              v-for="job in jobs"
              :key="job.id"
              :class="['job-row', { active: selected?.id === job.id }]"
              @click="selected = job"
            >
              <span class="job-icon">{{ statusIcon(job.status) }}</span>
              <span class="job-info">
                <span class="job-time">{{ formatDateTime(job.created_at) }}</span>
                <span class="job-meta">
                  <span v-if="job.scheduled_for">scheduled {{ formatDateTime(job.scheduled_for) }}</span>
                  <span v-else>manual run</span>
                </span>
              </span>
              <span class="job-side">
                <span :class="['badge', statusClass(job.status)]">{{ job.status }}</span>
                <span class="duration-cell">{{ formatDuration(job.duration_ms) }}</span>
              </span>
            </button>
          </div>
        </div>

        <div class="log-viewer">
          <div class="log-toolbar">
            <div>
              <div class="panel-title">Logs</div>
              <div class="panel-subtitle">
                <template v-if="selected">{{ selected.id }}</template>
                <template v-else>No run selected</template>
              </div>
            </div>
            <div class="detail-actions">
              <button class="btn sm" @click="copyLog">Copy</button>
              <button v-if="selected" class="btn sm" @click="rerun(selected)">Re-run</button>
            </div>
          </div>
          <pre class="log-output">{{ selectedLogContent }}</pre>
        </div>
      </section>
    </template>
  </div>
</template>

<style scoped>
.detail-page {
  display: flex;
  flex-direction: column;
  gap: 8px;
  min-width: 0;
  height: calc(100vh - 144px);
}
.topbar {
  display: flex;
  align-items: center;
  gap: 10px;
  min-height: 34px;
}
.title-wrap {
  flex: 1;
  min-width: 0;
}
.title-wrap h2 {
  margin: 0;
  font-size: 27px;
  font-weight: 740;
  letter-spacing: -0.035em;
  line-height: 1.12;
}
.subtitle {
  margin-top: 2px;
  font-size: 11px;
  color: var(--text-secondary);
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
  font-family: monospace;
}
.empty {
  background: var(--bg-card);
  border: 1px solid var(--border);
  border-radius: var(--radius);
  backdrop-filter: var(--glass-blur);
  -webkit-backdrop-filter: var(--glass-blur);
  box-shadow: var(--glass-shadow-soft), var(--glass-highlight);
}
.panel-title {
  font-size: 10px;
  font-weight: 700;
  color: var(--text-secondary);
  text-transform: uppercase;
  letter-spacing: 0.4px;
  white-space: nowrap;
}
.ok { color: var(--success); }
.err { color: var(--danger); }
.muted { color: var(--text-secondary); }
.count-chip {
  border: 1px solid var(--border);
  border-radius: 999px;
  background: var(--bg-soft);
  padding: 3px 8px;
  font-size: 10px;
  font-weight: 650;
  text-transform: uppercase;
  white-space: nowrap;
}
.history-shell {
  flex: 1;
  min-height: 0;
  display: grid;
  grid-template-columns: minmax(280px, 34%) minmax(0, 1fr);
  gap: 10px;
}
.history-panel,
.log-viewer {
  min-width: 0;
  min-height: 0;
  overflow: hidden;
}
.history-panel {
  flex: 1;
  min-height: 0;
  display: flex;
  flex-direction: column;
}
.panel-head {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: 10px;
  padding: 8px 10px 7px;
  border-bottom: 1px solid var(--border);
  background: linear-gradient(180deg, rgba(255,255,255,0.18), rgba(255,255,255,0));
}
.panel-subtitle {
  margin-top: 2px;
  font-size: 11px;
  color: var(--text-secondary);
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}
.job-row {
  display: flex;
  align-items: center;
  gap: 9px;
}
.job-list {
  flex: 1;
  min-height: 0;
  overflow-y: auto;
  padding: 3px 4px 5px;
}
.job-row {
  width: 100%;
  text-align: left;
  border: 1px solid transparent;
  border-radius: 7px;
  background: transparent;
  padding: 7px 8px;
  cursor: pointer;
}
.job-row + .job-row { margin-top: 2px; }
.job-row:hover {
  background: var(--bg-soft);
  border-color: var(--border);
}
.job-row.active {
  background: var(--bg-elevated);
  border-color: var(--accent-strong);
  box-shadow: var(--glass-highlight);
}
.job-icon {
  width: 20px;
  flex: 0 0 20px;
  text-align: center;
}
.job-info {
  flex: 1;
  min-width: 0;
  display: flex;
  flex-direction: column;
  gap: 2px;
}
.job-time {
  color: var(--text);
  font-size: 12px;
  font-weight: 560;
}
.job-meta {
  color: var(--text-secondary);
  font-size: 11px;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}
.duration-cell {
  color: var(--text-secondary);
  font-family: monospace;
  font-size: 11px;
  white-space: nowrap;
}
.job-side {
  flex: 0 0 auto;
  display: flex;
  flex-direction: column;
  align-items: flex-end;
  gap: 3px;
}
.log-viewer {
  display: flex;
  flex-direction: column;
}
.log-toolbar {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: 10px;
  padding: 8px 10px 7px;
  border-bottom: 1px solid var(--border);
  background: linear-gradient(180deg, rgba(255,255,255,0.18), rgba(255,255,255,0));
}
.detail-actions {
  display: flex;
  gap: 6px;
}
.log-output {
  flex: 1;
  min-height: 0;
  margin: 0;
  padding: 12px;
  overflow: auto;
  background: var(--bg-code);
  font-family: 'SF Mono', monospace;
  font-size: 12px;
  line-height: 1.6;
  white-space: pre-wrap;
  word-break: break-word;
  color: #e6e6ea;
}
.empty,
.empty-inline,
.empty-detail {
  color: var(--text-secondary);
  text-align: center;
  padding: 44px 16px;
}
.empty-detail {
  height: 100%;
  display: flex;
  align-items: center;
  justify-content: center;
}
.mono {
  min-width: 0;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
  font-family: monospace;
  font-size: 11px;
}
@media (max-width: 1200px) {
  .history-shell {
    grid-template-columns: minmax(240px, 38%) minmax(0, 1fr);
  }
}
@media (max-width: 760px) {
  .history-shell {
    grid-template-columns: 1fr;
    grid-template-rows: minmax(220px, 0.42fr) minmax(0, 1fr);
  }
  .duration-cell { display: none; }
}
</style>
