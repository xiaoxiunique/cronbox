<script setup lang="ts">
import { ref, onMounted } from "vue";
import { api, type Schedule } from "../lib/api";

const schedules = ref<Schedule[]>([]);

async function load() {
  schedules.value = await api.listSchedules();
}

async function toggle(id: string, enabled: boolean) {
  await api.setScheduleEnabled(id, enabled);
  await load();
}

async function remove(id: string) {
  if (!confirm("Delete this schedule?")) return;
  await api.deleteSchedule(id);
  await load();
}

function shortTime(iso: string | null) {
  if (!iso) return "—";
  const d = new Date(iso);
  const now = new Date();
  const diff = d.getTime() - now.getTime();
  const mins = Math.round(diff / 60000);
  if (mins > 0 && mins < 60) return `in ${mins}m`;
  if (mins >= 60) return `in ${Math.round(mins / 60)}h`;
  return new Date(iso).toLocaleString();
}

function detailUrl(schedule: Schedule) {
  return { path: "/scripts/detail", query: { path: schedule.script_path, baseDir: schedule.base_dir } };
}

onMounted(load);
</script>

<template>
  <div class="schedules-page">
    <div class="header">
      <h2>Schedules</h2>
      <button @click="load" class="btn-icon">↻</button>
    </div>

    <div v-if="schedules.length === 0" class="empty">
      <p>No schedules yet.</p>
      <p class="hint">Go to Scripts and click ⏰ to schedule a script.</p>
    </div>

    <div v-else class="list">
      <div v-for="s in schedules" :key="s.id" class="row">
        <label class="toggle">
          <input type="checkbox" :checked="s.enabled" @change="toggle(s.id, !s.enabled)" />
          <span class="slider"></span>
        </label>
        <div class="info">
          <router-link :to="detailUrl(s)" class="script-path script-link">{{ s.script_path }}</router-link>
          <div class="meta">
            <span class="base-dir">📁 {{ s.base_dir.split('/').pop() }}</span>
            <span class="cron">{{ s.cron_expr }}</span>
            <span class="tz">{{ s.timezone }}</span>
            <span class="next" v-if="s.next_run_at">Next: {{ shortTime(s.next_run_at) }}</span>
          </div>
        </div>
        <button @click="remove(s.id)" class="btn-icon del">✕</button>
      </div>
    </div>
  </div>
</template>

<style scoped>
.schedules-page { min-width: 0; }
.header { display: flex; align-items: center; gap: 12px; margin-bottom: 16px; }
.header h2 { flex: 1; font-size: 20px; margin: 0; }
.empty {
  text-align: center;
  padding: 48px 20px;
  color: var(--text-secondary);
  background: var(--bg-card);
  border: 1px solid var(--border);
  border-radius: 8px;
  backdrop-filter: var(--glass-blur);
  -webkit-backdrop-filter: var(--glass-blur);
  box-shadow: var(--glass-shadow-soft), var(--glass-highlight);
}
.empty p { margin: 4px 0; }
.hint { font-size: 12px; margin-top: 8px; }
.list { display: flex; flex-direction: column; gap: 6px; }
.row {
  display: flex;
  align-items: center;
  gap: 12px;
  padding: 12px;
  background:
    linear-gradient(180deg, rgba(255, 255, 255, 0.34), rgba(255, 255, 255, 0.10)),
    var(--bg-card);
  border-radius: 8px;
  border: 1px solid var(--border);
  transition: background 0.16s ease, border-color 0.16s ease, transform 0.16s ease, box-shadow 0.16s ease;
}
.row:hover {
  background: var(--bg-elevated);
  border-color: var(--border-strong);
  transform: translateY(-1px);
  box-shadow: var(--glass-shadow-soft), var(--glass-highlight);
}
.info { flex: 1; min-width: 0; }
.script-path { font-weight: 600; font-size: 14px; }
.script-link {
  display: inline-block;
  max-width: 100%;
  color: inherit;
  text-decoration: none;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
  border-radius: 7px;
}
.script-link:hover { color: var(--accent); }
.meta { display: flex; gap: 8px; flex-wrap: wrap; font-size: 11px; color: var(--text-secondary); margin-top: 4px; }
.base-dir,
.cron,
.tz,
.next {
  background: var(--bg-soft);
  border: 1px solid var(--border);
  border-radius: 999px;
  padding: 2px 7px;
}
.cron { font-family: monospace; }
.next { color: var(--warning); }
.btn-icon {
  border: 1px solid var(--border);
  background: var(--bg-soft);
  cursor: pointer;
  font-size: 16px;
  color: var(--text-secondary);
  border-radius: 8px;
  min-width: 30px;
  height: 30px;
}
.btn-icon:hover { color: var(--text); background: var(--bg-elevated); border-color: var(--border-strong); }
.del:hover { color: var(--danger); }

/* Toggle switch */
.toggle { position: relative; width: 40px; height: 22px; flex-shrink: 0; }
.toggle input { opacity: 0; width: 0; height: 0; }
.slider {
  position: absolute;
  inset: 0;
  background: var(--bg-soft);
  border: 1px solid var(--border);
  border-radius: 22px;
  cursor: pointer;
  transition: background 0.2s ease, border-color 0.2s ease, box-shadow 0.2s ease;
}
.slider::before {
  content: ''; position: absolute; width: 16px; height: 16px; border-radius: 50%;
  background: #fff; left: 2px; top: 2px; transition: transform 0.2s ease; box-shadow: 0 2px 7px rgba(31, 54, 82, 0.18);
}
.toggle input:checked + .slider { background: rgba(52, 199, 89, 0.76); border-color: rgba(52, 199, 89, 0.84); box-shadow: 0 0 0 3px rgba(52, 199, 89, 0.12); }
.toggle input:checked + .slider::before { transform: translateX(18px); }
@media (prefers-color-scheme: dark) {
  .row {
    background:
      linear-gradient(180deg, rgba(255, 255, 255, 0.08), rgba(255, 255, 255, 0.02)),
      var(--bg-card);
  }
}
</style>
