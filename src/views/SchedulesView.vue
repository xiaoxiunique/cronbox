<script setup lang="ts">
import { ref, onMounted } from "vue";
import { api, type Schedule, type ScriptFile } from "../lib/api";

const schedules = ref<Schedule[]>([]);
const scripts = ref<ScriptFile[]>([]);

async function load() {
  const [s, scs] = await Promise.all([api.listSchedules(), api.scanScripts()]);
  schedules.value = s;
  scripts.value = scs;
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

function aliasFor(s: Schedule): string {
  const match = scripts.value.find(
    (sc) => sc.base_dir === s.base_dir && sc.path === s.script_path
  );
  if (match?.alias) return match.alias;
  return s.script_path.split("/").pop() || s.script_path;
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
          <router-link :to="detailUrl(s)" class="alias script-link">{{ aliasFor(s) }}</router-link>
          <div class="meta">
            <span class="cron">{{ s.cron_expr }}</span>
            <span class="once-tag" v-if="s.one_shot">once</span>
          </div>
          <div class="full-path">{{ s.base_dir }}/{{ s.script_path }}</div>
        </div>
        <span class="next" v-if="s.next_run_at">{{ shortTime(s.next_run_at) }}</span>
        <button @click="remove(s.id)" class="btn-icon del">✕</button>
      </div>
    </div>
  </div>
</template>

<style scoped>
.schedules-page { min-width: 0; }
.header { display: flex; align-items: center; gap: 12px; margin-bottom: 18px; }
.header h2 { flex: 1; font-size: 32px; font-weight: 760; letter-spacing: -0.04em; margin: 0; }
.empty {
  text-align: center;
  padding: 48px 20px;
  color: var(--text-secondary);
  background: var(--bg-card);
  border: 1px solid var(--border);
  border-radius: var(--radius);
  backdrop-filter: var(--glass-blur);
  -webkit-backdrop-filter: var(--glass-blur);
  box-shadow: var(--glass-shadow-soft), var(--glass-highlight);
}
.empty p { margin: 4px 0; }
.hint { font-size: 12px; margin-top: 8px; }
.list {
  display: flex;
  flex-direction: column;
  background: var(--bg-card);
  border: 1px solid var(--border);
  border-radius: var(--radius-lg);
  box-shadow: var(--card-shadow);
  backdrop-filter: var(--glass-blur);
  -webkit-backdrop-filter: var(--glass-blur);
  overflow: hidden;
}
.row {
  display: flex;
  align-items: center;
  gap: 12px;
  padding: 12px 16px;
  transition: background 0.16s ease;
}
.row + .row { border-top: 1px solid var(--border); }
.row:hover { background: var(--bg-soft); }
.info { flex: 1; min-width: 0; }
.alias { font-weight: 600; font-size: 14px; }
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
.meta {
  display: flex;
  align-items: center;
  gap: 8px;
  margin-top: 3px;
  font-size: 11px;
  color: var(--text-secondary);
}
.cron { font-family: monospace; }
.once-tag {
  font-size: 10px;
  font-weight: 650;
  color: var(--accent);
  background: var(--accent-soft);
  border-radius: 999px;
  padding: 1px 6px;
  letter-spacing: 0.3px;
  text-transform: uppercase;
}
.full-path {
  font-size: 11px;
  color: var(--text-secondary);
  font-family: monospace;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
  max-height: 0;
  opacity: 0;
  margin-top: 0;
  transition: opacity 0.15s ease, max-height 0.15s ease, margin-top 0.15s ease;
}
.row:hover .full-path {
  max-height: 16px;
  opacity: 1;
  margin-top: 3px;
}
.next {
  font-size: 12px;
  color: var(--warning);
  font-weight: 600;
  white-space: nowrap;
  flex-shrink: 0;
}
.btn-icon { font-size: 15px; }
.del:hover { color: var(--danger); border-color: rgba(255, 59, 48, 0.24); }

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
</style>
