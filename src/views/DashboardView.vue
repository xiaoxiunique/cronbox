<script setup lang="ts">
import { computed, onMounted, onUnmounted, ref } from "vue";
import { api, type DashboardStats, type Schedule, type Job } from "../lib/api";

const stats = ref<DashboardStats | null>(null);
const schedules = ref<Schedule[]>([]);
const jobs = ref<Job[]>([]);
const loading = ref(true);
const error = ref("");
let refreshTimer: number | null = null;

const emptyRecent = {
  total: 0,
  success: 0,
  failure: 0,
  running: 0,
  queued: 0,
  cancelled: 0,
  skipped: 0,
};

const recent = computed(() => stats.value?.recent_runs ?? emptyRecent);
const scriptTotal = computed(() => stats.value?.script_total ?? 0);
const scheduleTotal = computed(() => stats.value?.schedule_total ?? 0);
const enabledTotal = computed(() => stats.value?.enabled_schedule_total ?? 0);
const activeNow = computed(() => recent.value.running + recent.value.queued);

const successRate = computed(() => {
  if (recent.value.total === 0) return 0;
  return Math.round((recent.value.success / recent.value.total) * 100);
});

const runMixStyle = computed(() => {
  const total = Math.max(1, recent.value.total);
  return {
    "--success-width": `${(recent.value.success / total) * 100}%`,
    "--failure-width": `${(recent.value.failure / total) * 100}%`,
    "--active-width": `${(activeNow.value / total) * 100}%`,
  };
});

const upcoming = computed(() =>
  schedules.value
    .filter((s) => s.enabled && s.next_run_at)
    .sort((a, b) => (a.next_run_at ?? "").localeCompare(b.next_run_at ?? ""))
    .slice(0, 6),
);

const recentJobs = computed(() =>
  [...jobs.value]
    .sort((a, b) => b.created_at.localeCompare(a.created_at))
    .slice(0, 8),
);

const nextRunLabel = computed(() => {
  const next = upcoming.value[0]?.next_run_at;
  return next ? relTime(next) : "none scheduled";
});

function baseName(path: string): string {
  return path.split("/").pop() || path;
}

function detailUrl(scriptPath: string, baseDir: string) {
  return { path: "/scripts/detail", query: { path: scriptPath, baseDir } };
}

function relTime(iso: string | null): string {
  if (!iso) return "—";
  const t = new Date(iso).getTime();
  if (Number.isNaN(t)) return "—";
  const diff = t - Date.now();
  const abs = Math.abs(diff);
  if (abs < 45000) return diff >= 0 ? "soon" : "just now";
  const m = Math.round(abs / 60000);
  let label: string;
  if (m < 60) label = `${m}m`;
  else if (m < 1440) label = `${Math.round(m / 60)}h`;
  else label = `${Math.round(m / 1440)}d`;
  return diff >= 0 ? `in ${label}` : `${label} ago`;
}

function absTime(iso: string | null): string {
  if (!iso) return "";
  const d = new Date(iso);
  if (Number.isNaN(d.getTime())) return "";
  return d.toLocaleString([], {
    month: "short",
    day: "numeric",
    hour: "2-digit",
    minute: "2-digit",
  });
}

function fmtDuration(ms: number | null): string {
  if (ms == null) return "";
  if (ms < 1000) return `${ms}ms`;
  const s = ms / 1000;
  if (s < 60) return `${s.toFixed(s < 10 ? 1 : 0)}s`;
  const m = Math.floor(s / 60);
  return `${m}m${Math.round(s % 60)}s`;
}

function statusMeta(status: string): { label: string; cls: string } {
  switch (status) {
    case "success":
      return { label: "Success", cls: "ok" };
    case "failure":
      return { label: "Failure", cls: "err" };
    case "interrupted":
      return { label: "Interrupted", cls: "err" };
    case "running":
      return { label: "Running", cls: "active" };
    case "queued":
      return { label: "Queued", cls: "active" };
    case "skipped":
      return { label: "Skipped", cls: "muted" };
    case "cancelled":
      return { label: "Cancelled", cls: "muted" };
    default:
      return { label: status, cls: "muted" };
  }
}

function jobTime(job: Job): string | null {
  return job.completed_at ?? job.started_at ?? job.created_at;
}

async function load() {
  try {
    error.value = "";
    const [s, sch, j] = await Promise.all([
      api.dashboardStats(),
      api.listSchedules(),
      api.listJobs(12),
    ]);
    stats.value = s;
    schedules.value = sch;
    jobs.value = j;
  } catch (e: any) {
    error.value = String(e);
  } finally {
    loading.value = false;
  }
}

onMounted(() => {
  load();
  refreshTimer = window.setInterval(load, 5000);
});

onUnmounted(() => {
  if (refreshTimer !== null) clearInterval(refreshTimer);
});
</script>

<template>
  <div class="dashboard">
    <header class="head">
      <div class="head-copy">
        <h1>Home</h1>
        <p class="status-line">
          {{ scriptTotal }} scripts · {{ enabledTotal }}/{{ scheduleTotal }} schedules enabled ·
          next run <strong>{{ nextRunLabel }}</strong>
        </p>
      </div>
      <button class="btn-icon" title="Refresh" @click="load">↻</button>
    </header>

    <div v-if="error" class="error">{{ error }}</div>

    <section class="kpis">
      <article class="kpi">
        <span class="kpi-label">Scripts</span>
        <strong>{{ loading ? "—" : scriptTotal }}</strong>
        <span class="kpi-note">runnable entrypoints</span>
      </article>
      <article class="kpi">
        <span class="kpi-label">Schedules</span>
        <strong>{{ loading ? "—" : enabledTotal }}<small>/{{ scheduleTotal }}</small></strong>
        <span class="kpi-note">enabled / total</span>
      </article>
      <article class="kpi">
        <span class="kpi-label">Runs / 24h</span>
        <strong>{{ loading ? "—" : recent.total }}</strong>
        <span class="kpi-note">{{ activeNow }} active now</span>
      </article>
      <article class="kpi">
        <span class="kpi-label">Success / 24h</span>
        <strong :class="{ good: successRate >= 90, warn: recent.failure > 0 }">
          {{ loading ? "—" : successRate }}<small v-if="!loading">%</small>
        </strong>
        <div class="mix" :style="runMixStyle" :title="`${recent.success} ok · ${recent.failure} failed · ${activeNow} active`">
          <span class="mix-success"></span>
          <span class="mix-failure"></span>
          <span class="mix-active"></span>
        </div>
      </article>
    </section>

    <section class="panels">
      <article class="panel">
        <div class="panel-head">
          <h2>Upcoming runs</h2>
          <span class="count">{{ upcoming.length }}</span>
        </div>
        <ul v-if="upcoming.length" class="list">
          <router-link v-for="s in upcoming" :key="s.id" :to="detailUrl(s.script_path, s.base_dir)" class="row up">
            <div class="row-main">
              <span class="name">{{ baseName(s.script_path) }}</span>
              <span class="sub">{{ s.cron_expr }}</span>
            </div>
            <div class="row-right">
              <span class="rel">{{ relTime(s.next_run_at) }}</span>
              <span class="abs">{{ absTime(s.next_run_at) }}</span>
            </div>
          </router-link>
        </ul>
        <div v-else class="empty">No enabled schedules yet.</div>
      </article>

      <article class="panel">
        <div class="panel-head">
          <h2>Recent activity</h2>
          <span class="count">{{ recentJobs.length }}</span>
        </div>
        <ul v-if="recentJobs.length" class="list">
          <router-link v-for="job in recentJobs" :key="job.id" :to="detailUrl(job.script_path, job.base_dir)" class="row job">
            <span class="pill" :class="statusMeta(job.status).cls">{{ statusMeta(job.status).label }}</span>
            <div class="row-main">
              <span class="name">{{ baseName(job.script_path) }}</span>
              <span v-if="job.error" class="sub err-text">{{ job.error }}</span>
            </div>
            <div class="row-right">
              <span class="rel">{{ relTime(jobTime(job)) }}</span>
              <span class="abs">{{ fmtDuration(job.duration_ms) }}</span>
            </div>
          </router-link>
        </ul>
        <div v-else class="empty">No runs recorded yet.</div>
      </article>
    </section>
  </div>
</template>

<style scoped>
.dashboard {
  display: flex;
  flex-direction: column;
  gap: 14px;
  min-width: 0;
}

.head {
  display: flex;
  align-items: flex-start;
  justify-content: space-between;
  gap: 16px;
}
.head h1 {
  margin: 0;
  font-size: 34px;
  font-weight: 760;
  letter-spacing: -0.045em;
}
.status-line {
  margin: 4px 0 0;
  font-size: 13px;
  color: var(--text-secondary);
}
.status-line strong {
  color: var(--text);
  font-weight: 600;
}
.btn-icon {
  flex: none;
}

.error {
  padding: 10px 14px;
  border-radius: var(--radius-sm);
  border: 1px solid rgba(255, 59, 48, 0.3);
  background: rgba(255, 59, 48, 0.08);
  color: var(--danger);
  font-size: 13px;
}

.kpis {
  display: grid;
  grid-template-columns: repeat(4, 1fr);
  gap: 12px;
}
.kpi {
  padding: 16px 18px;
  display: flex;
  flex-direction: column;
  gap: 4px;
  min-width: 0;
}
.kpi-label {
  font-size: 12px;
  color: var(--text-secondary);
}
.kpi strong {
  font-size: 26px;
  font-weight: 700;
  letter-spacing: -0.02em;
  line-height: 1.1;
}
.kpi strong small {
  font-size: 15px;
  font-weight: 600;
  color: var(--text-secondary);
}
.kpi strong.good {
  color: var(--success);
}
.kpi strong.warn {
  color: var(--text);
}
.kpi-note {
  font-size: 11.5px;
  color: var(--text-secondary);
}
.mix {
  display: flex;
  height: 5px;
  margin-top: 6px;
  border-radius: 999px;
  overflow: hidden;
  background: var(--bg-soft);
}
.mix span {
  height: 100%;
}
.mix-success {
  width: var(--success-width);
  background: var(--success);
}
.mix-failure {
  width: var(--failure-width);
  background: var(--danger);
}
.mix-active {
  width: var(--active-width);
  background: var(--accent);
}

.panels {
  display: grid;
  grid-template-columns: 1fr 1fr;
  gap: 12px;
}
.panel {
  padding: 6px 6px 8px;
  min-width: 0;
}
.panel-head {
  display: flex;
  align-items: center;
  justify-content: space-between;
  padding: 10px 12px 8px;
}
.panel-head h2 {
  margin: 0;
  font-size: 13px;
  font-weight: 650;
  color: var(--text);
}
.count {
  font-size: 11px;
  font-weight: 600;
  color: var(--text-secondary);
  background: var(--bg-soft);
  border-radius: 999px;
  padding: 1px 8px;
}

.list {
  list-style: none;
  margin: 0;
  padding: 0;
}
.row {
  display: flex;
  align-items: center;
  gap: 10px;
  padding: 8px 12px;
  border-radius: var(--radius-sm);
  text-decoration: none;
  color: inherit;
  cursor: pointer;
}
.row + .row {
  border-top: 1px solid var(--border);
  border-radius: 0;
}
.row:hover {
  background: var(--bg-soft);
}
.row-main {
  flex: 1;
  min-width: 0;
  display: flex;
  flex-direction: column;
  gap: 2px;
}
.name {
  font-size: 13px;
  font-weight: 550;
  white-space: nowrap;
  overflow: hidden;
  text-overflow: ellipsis;
}
.sub {
  font-size: 11.5px;
  color: var(--text-secondary);
  font-variant-numeric: tabular-nums;
  white-space: nowrap;
  overflow: hidden;
  text-overflow: ellipsis;
}
.sub.err-text {
  color: var(--danger);
}
.row-right {
  flex: none;
  display: flex;
  flex-direction: column;
  align-items: flex-end;
  gap: 2px;
}
.rel {
  font-size: 12.5px;
  font-weight: 550;
  color: var(--text);
  font-variant-numeric: tabular-nums;
}
.abs {
  font-size: 11px;
  color: var(--text-secondary);
  font-variant-numeric: tabular-nums;
}

.pill {
  flex: none;
  width: 74px;
}

.empty {
  padding: 22px 12px;
  text-align: center;
  font-size: 13px;
  color: var(--text-secondary);
}

@media (max-width: 900px) {
  .kpis {
    grid-template-columns: repeat(2, 1fr);
  }
  .panels {
    grid-template-columns: 1fr;
  }
}
</style>
