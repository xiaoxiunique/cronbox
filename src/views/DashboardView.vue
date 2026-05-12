<script setup lang="ts">
import { computed, onMounted, onUnmounted, ref } from "vue";
import { api, type DashboardStats, type ScheduleDistributionBucket } from "../lib/api";

const stats = ref<DashboardStats | null>(null);
const loading = ref(true);
const error = ref("");
let refreshTimer: number | null = null;

const emptyStats: DashboardStats = {
  script_total: 0,
  schedule_total: 0,
  enabled_schedule_total: 0,
  recent_runs: {
    total: 0,
    success: 0,
    failure: 0,
    running: 0,
    queued: 0,
    cancelled: 0,
    skipped: 0,
  },
  schedule_distribution: Array.from({ length: 24 }, (_, hour) => ({
    hour: `${String(hour).padStart(2, "0")}:00`,
    count: 0,
  })),
};

const viewStats = computed(() => stats.value ?? emptyStats);
const recent = computed(() => viewStats.value.recent_runs);
const scheduleBuckets = computed(() => viewStats.value.schedule_distribution);
const maxBucket = computed(() =>
  Math.max(1, ...scheduleBuckets.value.map((bucket) => bucket.count)),
);
const activeBuckets = computed(() => scheduleBuckets.value.filter((bucket) => bucket.count > 0));
const successRate = computed(() => {
  if (recent.value.total === 0) return 0;
  return Math.round((recent.value.success / recent.value.total) * 100);
});
const failureRate = computed(() => {
  if (recent.value.total === 0) return 0;
  return Math.round((recent.value.failure / recent.value.total) * 100);
});
const runMixStyle = computed(() => {
  const total = Math.max(1, recent.value.total);
  const success = (recent.value.success / total) * 100;
  const failure = (recent.value.failure / total) * 100;
  const active = ((recent.value.running + recent.value.queued) / total) * 100;
  return {
    "--success-width": `${success}%`,
    "--failure-width": `${failure}%`,
    "--active-width": `${active}%`,
  };
});

async function load() {
  try {
    error.value = "";
    stats.value = await api.dashboardStats();
  } catch (e: any) {
    error.value = String(e);
  } finally {
    loading.value = false;
  }
}

function barHeight(bucket: ScheduleDistributionBucket) {
  return `${Math.max(8, Math.round((bucket.count / maxBucket.value) * 100))}%`;
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
    <header class="hero glass-card">
      <div class="hero-copy">
        <div class="eyebrow">Local scheduler control room</div>
        <h1>CronBox at a glance</h1>
        <p>
          A compact overview of runnable scripts, recent execution health, and where current
          schedules are concentrated.
        </p>
      </div>
      <button class="btn-icon refresh" title="Refresh dashboard" @click="load">↻</button>
    </header>

    <div v-if="error" class="error">{{ error }}</div>

    <section class="metrics">
      <article class="metric glass-card">
        <span class="metric-label">Scripts</span>
        <strong>{{ loading ? "—" : viewStats.script_total }}</strong>
        <span class="metric-note">runnable entrypoints</span>
      </article>
      <article class="metric glass-card">
        <span class="metric-label">Runs / 24h</span>
        <strong>{{ loading ? "—" : recent.total }}</strong>
        <span class="metric-note">{{ recent.running + recent.queued }} active now</span>
      </article>
      <article class="metric glass-card success">
        <span class="metric-label">Success / 24h</span>
        <strong>{{ loading ? "—" : recent.success }}</strong>
        <span class="metric-note">{{ successRate }}% of recent runs</span>
      </article>
      <article class="metric glass-card danger">
        <span class="metric-label">Failure / 24h</span>
        <strong>{{ loading ? "—" : recent.failure }}</strong>
        <span class="metric-note">{{ failureRate }}% of recent runs</span>
      </article>
    </section>

    <section class="overview-grid">
      <article class="run-health glass-card">
        <div class="section-head">
          <div>
            <h2>Recent run health</h2>
            <p>Last 24 hours across manual and scheduled jobs.</p>
          </div>
        </div>

        <div class="run-mix" :style="runMixStyle">
          <span class="mix-success"></span>
          <span class="mix-failure"></span>
          <span class="mix-active"></span>
        </div>

        <div class="status-grid">
          <div>
            <span class="status-dot ok"></span>
            <span>Success</span>
            <strong>{{ recent.success }}</strong>
          </div>
          <div>
            <span class="status-dot err"></span>
            <span>Failure</span>
            <strong>{{ recent.failure }}</strong>
          </div>
          <div>
            <span class="status-dot active"></span>
            <span>Running / queued</span>
            <strong>{{ recent.running + recent.queued }}</strong>
          </div>
          <div>
            <span class="status-dot muted"></span>
            <span>Skipped / cancelled</span>
            <strong>{{ recent.skipped + recent.cancelled }}</strong>
          </div>
        </div>
      </article>

      <article class="schedule-chart glass-card">
        <div class="section-head">
          <div>
            <h2>Schedule distribution</h2>
            <p>
              Enabled schedules grouped by the hour of their next run. {{ viewStats.enabled_schedule_total }}
              of {{ viewStats.schedule_total }} schedules are enabled.
            </p>
          </div>
        </div>

        <div class="chart" aria-label="Enabled schedule distribution by next run hour">
          <div
            v-for="bucket in scheduleBuckets"
            :key="bucket.hour"
            class="bar-wrap"
            :title="`${bucket.hour}: ${bucket.count}`"
          >
            <div class="bar-track">
              <div class="bar" :style="{ height: barHeight(bucket) }" :class="{ empty: bucket.count === 0 }"></div>
            </div>
            <span v-if="bucket.hour.endsWith('00') && Number(bucket.hour.slice(0, 2)) % 6 === 0">
              {{ bucket.hour.slice(0, 2) }}
            </span>
          </div>
        </div>

        <div v-if="activeBuckets.length" class="bucket-list">
          <span v-for="bucket in activeBuckets" :key="bucket.hour">
            {{ bucket.hour }} · {{ bucket.count }}
          </span>
        </div>
        <div v-else class="empty-chart">No enabled schedules yet.</div>
      </article>
    </section>
  </div>
</template>

<style scoped>
.dashboard {
  display: flex;
  flex-direction: column;
  gap: 16px;
  min-width: 0;
}

.hero {
  display: flex;
  align-items: flex-start;
  justify-content: space-between;
  gap: 18px;
  min-height: 156px;
  padding: 24px;
  border-radius: 8px;
  background:
    linear-gradient(135deg, rgba(22, 119, 255, 0.18), rgba(52, 199, 89, 0.07)),
    var(--bg-card);
  overflow: hidden;
  position: relative;
}

.hero::after {
  content: "";
  position: absolute;
  inset: auto 22px 20px auto;
  width: 160px;
  height: 72px;
  border: 1px solid var(--border);
  border-radius: 8px;
  background:
    linear-gradient(90deg, transparent 0 18%, rgba(22, 119, 255, 0.28) 18% 24%, transparent 24% 48%, rgba(52, 199, 89, 0.28) 48% 56%, transparent 56%),
    rgba(255, 255, 255, 0.10);
  opacity: 0.45;
  transform: skewX(-8deg);
}

.hero-copy {
  max-width: 620px;
  position: relative;
  z-index: 1;
}

.eyebrow {
  color: var(--accent);
  font-size: 12px;
  font-weight: 760;
  letter-spacing: 0.02em;
  margin-bottom: 12px;
  text-transform: uppercase;
}

h1,
h2,
p {
  margin: 0;
}

h1 {
  font-size: clamp(34px, 5vw, 58px);
  line-height: 0.96;
  letter-spacing: 0;
}

.hero p,
.section-head p,
.metric-note {
  color: var(--text-secondary);
}

.hero p {
  margin-top: 14px;
  max-width: 560px;
  line-height: 1.52;
}

.refresh {
  position: relative;
  z-index: 1;
}

.metrics {
  display: grid;
  grid-template-columns: repeat(4, minmax(0, 1fr));
  gap: 12px;
}

.metric {
  min-height: 128px;
  padding: 16px;
  border-radius: 8px;
  display: flex;
  flex-direction: column;
  justify-content: space-between;
}

.metric-label {
  color: var(--text-secondary);
  font-size: 12px;
  font-weight: 700;
}

.metric strong {
  display: block;
  font-size: 40px;
  line-height: 1;
  margin-top: 18px;
}

.metric.success strong {
  color: var(--success);
}

.metric.danger strong {
  color: var(--danger);
}

.metric-note {
  font-size: 12px;
  margin-top: 10px;
}

.overview-grid {
  display: grid;
  grid-template-columns: minmax(320px, 0.68fr) minmax(420px, 1fr);
  gap: 12px;
}

.run-health,
.schedule-chart {
  border-radius: 8px;
  padding: 18px;
}

.section-head {
  display: flex;
  justify-content: space-between;
  gap: 16px;
  margin-bottom: 18px;
}

.section-head h2 {
  font-size: 18px;
  line-height: 1.2;
}

.section-head p {
  margin-top: 6px;
  font-size: 13px;
  line-height: 1.45;
}

.run-mix {
  height: 12px;
  border-radius: 999px;
  overflow: hidden;
  display: flex;
  background: var(--bg-soft);
  border: 1px solid var(--border);
}

.run-mix span {
  min-width: 0;
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

.status-grid {
  display: grid;
  grid-template-columns: repeat(2, minmax(0, 1fr));
  gap: 10px;
  margin-top: 18px;
}

.status-grid div {
  display: grid;
  grid-template-columns: auto 1fr auto;
  align-items: center;
  gap: 8px;
  min-height: 42px;
  padding: 10px;
  border: 1px solid var(--border);
  border-radius: 8px;
  background: var(--bg-soft);
  font-size: 13px;
}

.status-grid strong {
  font-size: 18px;
}

.status-dot {
  width: 9px;
  height: 9px;
  border-radius: 999px;
}

.status-dot.ok {
  background: var(--success);
}

.status-dot.err {
  background: var(--danger);
}

.status-dot.active {
  background: var(--accent);
}

.status-dot.muted {
  background: var(--text-secondary);
}

.chart {
  display: grid;
  grid-template-columns: repeat(24, minmax(0, 1fr));
  gap: 5px;
  height: 190px;
  align-items: end;
  padding: 10px 0 0;
}

.bar-wrap {
  display: grid;
  grid-template-rows: 1fr 18px;
  min-width: 0;
  height: 100%;
}

.bar-track {
  display: flex;
  align-items: end;
  min-height: 0;
  border-radius: 999px;
  background: var(--bg-soft);
  overflow: hidden;
}

.bar {
  width: 100%;
  border-radius: 999px 999px 0 0;
  background: linear-gradient(180deg, var(--accent), rgba(52, 199, 89, 0.76));
  min-height: 8px;
  transition: height 0.22s ease;
}

.bar.empty {
  background: rgba(105, 113, 124, 0.22);
}

.bar-wrap span {
  align-self: end;
  color: var(--text-secondary);
  font-size: 10px;
  text-align: center;
}

.bucket-list {
  display: flex;
  flex-wrap: wrap;
  gap: 6px;
  margin-top: 14px;
}

.bucket-list span,
.empty-chart {
  border: 1px solid var(--border);
  border-radius: 999px;
  background: var(--bg-soft);
  color: var(--text-secondary);
  font-size: 11px;
  padding: 4px 8px;
}

.empty-chart {
  width: fit-content;
  margin-top: 14px;
}

.btn-icon {
  border: 1px solid var(--border);
  background: var(--bg-soft);
  cursor: pointer;
  font-size: 16px;
  color: var(--text-secondary);
  border-radius: 8px;
  min-width: 32px;
  height: 32px;
}

.btn-icon:hover {
  color: var(--text);
  background: var(--bg-elevated);
  border-color: var(--border-strong);
}

.error {
  border: 1px solid rgba(255, 59, 48, 0.24);
  border-radius: 8px;
  background: rgba(255, 59, 48, 0.08);
  color: var(--danger);
  padding: 10px 12px;
  font-size: 13px;
}

@media (max-width: 1040px) {
  .metrics {
    grid-template-columns: repeat(2, minmax(0, 1fr));
  }

  .overview-grid {
    grid-template-columns: 1fr;
  }
}

@media (max-width: 720px) {
  .hero {
    min-height: auto;
  }

  .hero::after {
    display: none;
  }

  .metrics,
  .status-grid {
    grid-template-columns: 1fr;
  }

  .chart {
    gap: 3px;
  }
}
</style>
