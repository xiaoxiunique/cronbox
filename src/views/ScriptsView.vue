<script setup lang="ts">
import { ref, computed, onMounted, onUnmounted, nextTick } from "vue";
import { api, type ScriptFile, type Schedule, type Job, type WorkDir, type ScriptParam } from "../lib/api";
import { open } from "@tauri-apps/plugin-dialog";
import LanguageLogo from "../components/LanguageLogo.vue";
import EnvEditor from "../components/EnvEditor.vue";
import { formatDuration } from "../lib/format";

const scripts = ref<ScriptFile[]>([]);
const schedules = ref<Schedule[]>([]);
const workDirs = ref<WorkDir[]>([]);

// Run panel
const runningJob = ref<Job | null>(null);
const runScriptPath = ref("");
const runBaseDir = ref("");
let pollTimer: number | null = null;
const logBox = ref<HTMLElement | null>(null);

// Script edit dialog
const showEditDialog = ref(false);
const editScript = ref<ScriptFile | null>(null);
const editSchedule = ref<Schedule | null>(null);
const editAlias = ref("");
const editScheduleEnabled = ref(false);
const editCronExpr = ref("0 * * * *");
const editTimezone = ref("Asia/Shanghai");
const editArgs = ref("{}");
const editEnvPairs = ref<{ key: string; value: string }[]>([]);
const editOneShot = ref(false);
const editCronError = ref("");
const editError = ref("");
const editUpcoming = ref<string[]>([]);
const savingEdit = ref(false);
const editWritesSchedule = computed(() => Boolean(editSchedule.value || editScheduleEnabled.value));
const editScheduleFieldsDisabled = computed(() => Boolean(!editSchedule.value && !editScheduleEnabled.value));

// Agent task dialog
type AgentKind = "codex" | "claude";
const showAgentDialog = ref(false);
const agentKind = ref<AgentKind>("codex");
const agentName = ref("daily-codex-task");
const agentPrompt = ref("Summarize the current repository status and suggest the next concrete action.");
const agentWorkspace = ref<WorkDir | null>(null);
const agentBaseDir = ref("");
const agentBrowsedDirs = ref<string[]>([]);
const creatingAgent = ref(false);
const preparingWorkspace = ref(false);

function isAgentTask(s: ScriptFile): boolean {
  return s.path.startsWith("cronbox/codex/") || s.path.startsWith("cronbox/claude/");
}

const taskSections = computed(() => {
  const sections: { kind: string; title: string; scripts: ScriptFile[] }[] = [];
  const agents = scripts.value.filter(isAgentTask);
  if (agents.length > 0) sections.push({ kind: "agent", title: "Agent Tasks", scripts: agents });
  const codes = scripts.value.filter((s) => !isAgentTask(s));
  if (codes.length > 0) sections.push({ kind: "code", title: "Code Scripts", scripts: codes });
  return sections;
});

async function load() {
  workDirs.value = await api.listWorkDirs();
  scripts.value = await api.scanScripts();
  schedules.value = await api.listSchedules();
}

const agentTitle = computed(() => agentKind.value === "codex" ? "Codex Task" : "Claude Task");
const agentCommand = computed(() => agentKind.value === "codex" ? "codex exec" : "claude -p");
const agentFolder = computed(() => agentKind.value === "codex" ? "codex" : "claude");
const agentDirChoices = computed(() => {
  const paths = new Set<string>(workDirs.value.map((d) => d.path));
  for (const d of agentBrowsedDirs.value) paths.add(d);
  if (agentBaseDir.value) paths.add(agentBaseDir.value);
  return Array.from(paths);
});

async function openAgentDialog(kind: AgentKind) {
  agentKind.value = kind;
  agentName.value = kind === "codex" ? "daily-codex-task" : "daily-claude-task";
  agentPrompt.value = kind === "codex"
    ? "Summarize the current repository status and suggest the next concrete action."
    : "Inspect this workspace and write a concise status note with the next concrete action.";
  preparingWorkspace.value = true;
  try {
    agentWorkspace.value = await api.ensureAgentWorkspace();
    await load();
    agentBrowsedDirs.value = [];
    agentBaseDir.value = agentWorkspace.value?.path ?? "";
    showAgentDialog.value = true;
  } catch (e: any) {
    alert(e);
  } finally {
    preparingWorkspace.value = false;
  }
}

async function browseAgentDir() {
  try {
    const selected = await open({ directory: true, multiple: false });
    if (!selected) return;
    const path = selected as string;
    if (!agentBrowsedDirs.value.includes(path)) agentBrowsedDirs.value.push(path);
    agentBaseDir.value = path;
  } catch (e: any) {
    alert(e);
  }
}

async function createAgentTask() {
  if (!agentName.value.trim() || !agentPrompt.value.trim()) return;
  creatingAgent.value = true;
  try {
    const create = agentKind.value === "codex" ? api.createCodexTask : api.createClaudeTask;
    const created = await create(agentName.value.trim(), agentPrompt.value, agentBaseDir.value);
    showAgentDialog.value = false;
    await load();
    const job = await api.runNow(created.script.path, created.script.base_dir);
    runScriptPath.value = created.script.path;
    runBaseDir.value = created.script.base_dir;
    runningJob.value = job;
    startPolling(job.id);
  } catch (e: any) {
    alert(e);
  } finally {
    creatingAgent.value = false;
  }
}

// Run dialog
const showRunDialog = ref(false);
const runDialogScript = ref<ScriptFile | null>(null);
const runArgs = ref("{}");
const detectedParams = ref<ScriptParam[]>([]);
const paramValues = ref<Record<string, string>>({});
const detectingArgs = ref(false);
const useFormMode = ref(false);

async function openRunDialog(s: ScriptFile) {
  runDialogScript.value = s;
  runArgs.value = "{}";
  detectedParams.value = [];
  paramValues.value = {};
  detectingArgs.value = true;
  useFormMode.value = false;
  showRunDialog.value = true;

  try {
    const params = await api.detectArgs(s.path, s.base_dir);
    detectedParams.value = params;
    if (params.length > 0) {
      useFormMode.value = true;
      // Pre-fill defaults
      for (const p of params) {
        paramValues.value[p.name] = p.default ?? "";
      }
    }
  } catch (e) {
    console.warn("Could not detect args:", e);
  } finally {
    detectingArgs.value = false;
  }
}

function buildArgsFromForm(): string {
  const args: Record<string, any> = {};
  for (const p of detectedParams.value) {
    const val = paramValues.value[p.name];
    if (val === "" || val === undefined) continue;
    // Convert types
    if (p.param_type === "bool") {
      args[p.name] = val === "true";
    } else if (p.param_type === "int") {
      args[p.name] = parseInt(val) || 0;
    } else {
      args[p.name] = val;
    }
  }
  return JSON.stringify(args);
}

async function confirmRun() {
  if (!runDialogScript.value) return;
  showRunDialog.value = false;
  const s = runDialogScript.value;
  const argsStr = useFormMode.value ? buildArgsFromForm() : runArgs.value;
  try {
    const job = await api.runNow(s.path, s.base_dir, argsStr);
    runScriptPath.value = s.path;
    runBaseDir.value = s.base_dir;
    runningJob.value = job;
    startPolling(job.id);
  } catch (e: any) { alert(e); }
}

async function quickRun(s: ScriptFile) {
  try {
    const job = await api.runNow(s.path, s.base_dir);
    runScriptPath.value = s.path;
    runBaseDir.value = s.base_dir;
    runningJob.value = job;
    startPolling(job.id);
  } catch (e: any) { alert(e); }
}

function startPolling(jobId: string) {
  stopPolling();
  pollTimer = window.setInterval(async () => {
    try {
      const job = await api.getJob(jobId);
      runningJob.value = job;
      await nextTick();
      if (logBox.value) logBox.value.scrollTop = logBox.value.scrollHeight;
      if (job.status !== "queued" && job.status !== "running") stopPolling();
    } catch { stopPolling(); }
  }, 500);
}

function stopPolling() {
  if (pollTimer !== null) { clearInterval(pollTimer); pollTimer = null; }
}

function closeRunPanel() {
  stopPolling();
  runningJob.value = null;
}

function statusIcon(s: string) {
  return { queued: "⏳", running: "🔄", success: "✅", failure: "❌", cancelled: "⛔" }[s] || "❓";
}

function envToPairs(json: string): { key: string; value: string }[] {
  try {
    const obj = JSON.parse(json || "{}");
    if (obj && typeof obj === "object" && !Array.isArray(obj)) {
      return Object.entries(obj).map(([key, value]) => ({ key, value: String(value) }));
    }
  } catch {
    // fall through
  }
  return [];
}

function pairsToJson(pairs: { key: string; value: string }[]): string {
  const obj: Record<string, string> = {};
  for (const { key, value } of pairs) {
    const k = key.trim();
    if (k) obj[k] = value;
  }
  return JSON.stringify(obj);
}

function openEditDialog(s: ScriptFile) {
  const schedule = getSchedule(s.path, s.base_dir) ?? null;
  editScript.value = s;
  editSchedule.value = schedule;
  editAlias.value = s.alias;
  editScheduleEnabled.value = schedule?.enabled ?? false;
  editCronExpr.value = schedule?.cron_expr ?? "0 * * * *";
  editTimezone.value = schedule?.timezone ?? "Asia/Shanghai";
  editArgs.value = schedule?.args ?? "{}";
  editEnvPairs.value = envToPairs(schedule?.env ?? "{}");
  editOneShot.value = schedule?.one_shot ?? false;
  editCronError.value = "";
  editError.value = "";
  editUpcoming.value = [];
  showEditDialog.value = true;
  checkEditCron();
}

async function checkEditCron() {
  if (!editCronExpr.value.trim()) {
    editCronError.value = "Cron expression is required.";
    editUpcoming.value = [];
    return;
  }
  try {
    await api.validateCron(editCronExpr.value);
    editCronError.value = "";
    editUpcoming.value = await api.upcomingRuns(editCronExpr.value, editTimezone.value);
  } catch (e: any) { editCronError.value = String(e); editUpcoming.value = []; }
}

async function saveEdit() {
  if (!editScript.value) return;
  const script = editScript.value;
  const scheduleArgs = editArgs.value.trim() || "{}";

  editError.value = "";
  if (editWritesSchedule.value) {
    await checkEditCron();
    if (editCronError.value) return;
    try {
      JSON.parse(scheduleArgs);
    } catch {
      editError.value = "Arguments must be valid JSON.";
      return;
    }
  }

  savingEdit.value = true;
  try {
    await api.setScriptAlias(script.base_dir, script.path, editAlias.value.trim() || undefined);
    const envJson = pairsToJson(editEnvPairs.value);
    if (editSchedule.value) {
      const updated = await api.updateSchedule(
        editSchedule.value.id,
        editCronExpr.value,
        editTimezone.value,
        scheduleArgs,
        envJson,
        editOneShot.value
      );
      await api.setScheduleEnabled(updated.id, editScheduleEnabled.value);
    } else if (editScheduleEnabled.value) {
      await api.createSchedule(script.path, script.base_dir, editCronExpr.value, editTimezone.value, scheduleArgs, envJson, editOneShot.value);
    }
    showEditDialog.value = false;
    await load();
  } catch (e: any) {
    editError.value = String(e);
  } finally {
    savingEdit.value = false;
  }
}

function getSchedule(path: string, baseDir: string): Schedule | undefined {
  return schedules.value.find(s => s.script_path === path && s.base_dir === baseDir);
}

function shortTime(iso: string | null) {
  if (!iso) return "";
  const diff = new Date(iso).getTime() - Date.now();
  const mins = Math.round(diff / 60000);
  if (Math.abs(mins) < 1) return "< 1min";
  return mins > 0 ? `in ${mins}m` : `${-mins}m ago`;
}

function detailUrl(s: ScriptFile) {
  return { path: "/scripts/detail", query: { path: s.path, baseDir: s.base_dir } };
}

onMounted(load);
onUnmounted(stopPolling);
</script>

<template>
  <div class="scripts-page">
    <div class="header">
      <h2>Scripts</h2>
      <button @click="openAgentDialog('codex')" class="btn agent-btn" :disabled="preparingWorkspace">
        {{ preparingWorkspace && agentKind === 'codex' ? 'Preparing...' : 'Codex Task' }}
      </button>
      <button @click="openAgentDialog('claude')" class="btn agent-btn claude-btn" :disabled="preparingWorkspace">
        {{ preparingWorkspace && agentKind === 'claude' ? 'Preparing...' : 'Claude Task' }}
      </button>
      <button @click="load" class="btn-icon" title="Refresh">↻</button>
    </div>

    <div v-if="workDirs.length === 0" class="empty">
      <p>No working directories configured.</p>
      <p>Go to <router-link to="/settings">Settings</router-link> to add one.</p>
    </div>

    <div v-else-if="scripts.length === 0" class="empty">
      <p>No scripts found in {{ workDirs.length }} director{{ workDirs.length > 1 ? 'ies' : 'y' }}.</p>
      <p class="hint">Supported: .py .sh .ts .js .sql</p>
    </div>

    <template v-else>
      <section v-for="section in taskSections" :key="section.kind" class="task-section">
        <div class="task-section-head">
          <span>{{ section.title }}</span>
          <span class="task-section-count">{{ section.scripts.length }}</span>
        </div>
        <div class="list">
          <div v-for="s in section.scripts" :key="s.base_dir + '/' + s.path"
            :class="['row', { active: runScriptPath === s.path && runBaseDir === s.base_dir && runningJob }]">
            <LanguageLogo :language="s.language" />
            <router-link :to="detailUrl(s)" class="info script-link">
              <div class="name">{{ s.alias }}</div>
              <div class="full-path">{{ s.base_dir }}/{{ s.path }}</div>
            </router-link>
            <button @click="openEditDialog(s)" class="btn config-btn" title="Edit script settings">⚙</button>
            <div class="schedule-badge" v-if="getSchedule(s.path, s.base_dir)">
              <span :class="['dot', getSchedule(s.path, s.base_dir)!.enabled ? 'on' : 'off']"></span>
              {{ getSchedule(s.path, s.base_dir)!.cron_expr }}
              <span class="once-tag" v-if="getSchedule(s.path, s.base_dir)!.one_shot">once</span>
              <span class="next" v-if="getSchedule(s.path, s.base_dir)!.next_run_at">
                {{ shortTime(getSchedule(s.path, s.base_dir)!.next_run_at) }}
              </span>
            </div>
            <button @click="openRunDialog(s)" class="btn run" title="Run with args"
              :disabled="runningJob?.status === 'running' && runScriptPath === s.path && runBaseDir === s.base_dir">
              {{ runningJob?.status === 'running' && runScriptPath === s.path && runBaseDir === s.base_dir ? '⟳' : '▶' }}
            </button>
            <button @click="quickRun(s)" class="btn quick-run" title="Quick run (no args)"
              :disabled="runningJob?.status === 'running' && runScriptPath === s.path && runBaseDir === s.base_dir">⚡</button>
          </div>
        </div>
      </section>

      <!-- Run output panel -->
      <div v-if="runningJob" class="run-panel">
        <div class="run-panel-header">
          <span class="run-status">{{ statusIcon(runningJob.status) }}</span>
          <span class="run-script">{{ runScriptPath }}</span>
          <span :class="['run-badge', `st-${runningJob.status}`]">{{ runningJob.status }}</span>
          <span class="run-duration" v-if="runningJob.duration_ms != null">{{ formatDuration(runningJob.duration_ms) }}</span>
          <span style="flex:1"></span>
          <button @click="closeRunPanel" class="btn-icon" title="Close">✕</button>
        </div>
        <div v-if="runningJob.error" class="run-error">{{ runningJob.error }}</div>
        <div class="run-logs" ref="logBox">
          <template v-if="runningJob.status === 'queued'"><span class="log-waiting">Waiting to start...</span></template>
          <template v-else-if="runningJob.logs">{{ runningJob.logs }}</template>
          <template v-else-if="runningJob.status === 'running'"><span class="log-waiting">Running...</span></template>
          <template v-else><span class="log-empty">(no output)</span></template>
        </div>
        <div v-if="runningJob.result" class="run-result">
          <span class="result-label">Result:</span> {{ runningJob.result }}
        </div>
      </div>
    </template>

    <!-- Edit Dialog -->
    <div v-if="showEditDialog" class="overlay" @click.self="showEditDialog = false">
      <div class="dialog edit-dialog">
        <h3>Edit Script</h3>
        <div class="edit-target">
          <LanguageLogo v-if="editScript" :language="editScript.language" />
          <div class="edit-target-text">
            <div class="edit-file">{{ editScript?.name }}</div>
            <div class="edit-path">{{ editScript?.path }}</div>
          </div>
        </div>
        <label>Alias</label>
        <input v-model="editAlias" placeholder="Display name" />
        <div class="hint">Leave empty to use the default alias from the filename.</div>

        <div class="schedule-edit-head">
          <div>
            <div class="section-title">Schedule</div>
            <div class="hint compact">{{ editSchedule ? 'Update this script schedule.' : 'Enable to create a schedule.' }}</div>
          </div>
          <label class="switch">
            <input type="checkbox" v-model="editScheduleEnabled" />
            <span class="switch-track"></span>
          </label>
        </div>

          <div class="schedule-form" :class="{ disabled: !editScheduleEnabled && !editSchedule }">
            <label>Cron Expression</label>
          <input
            v-model="editCronExpr"
            @input="checkEditCron"
            class="mono"
            placeholder="0 * * * *"
            :disabled="editScheduleFieldsDisabled"
          />
          <div v-if="editCronError" class="error">{{ editCronError }}</div>
          <label>Timezone</label>
          <input v-model="editTimezone" @input="checkEditCron" :disabled="editScheduleFieldsDisabled" />
          <label class="inline-check">
            <input type="checkbox" v-model="editOneShot" :disabled="editScheduleFieldsDisabled" />
            <span>Run once (disable after first run)</span>
          </label>
          <label>Arguments (JSON)</label>
          <textarea v-model="editArgs" class="mono" rows="4" :disabled="editScheduleFieldsDisabled"></textarea>
          <label>Environment Variables</label>
          <EnvEditor v-model="editEnvPairs" :disabled="editScheduleFieldsDisabled" />
          <div class="hint compact">Per-schedule env vars (API keys, etc). Applied on top of the resolved PATH.</div>
          <div v-if="editUpcoming.length" class="upcoming">
            <div class="upcoming-title">Next runs</div>
            <div v-for="t in editUpcoming" :key="t" class="upcoming-time">{{ t }}</div>
          </div>
        </div>

        <div v-if="editError" class="error">{{ editError }}</div>
        <div class="dialog-actions">
          <button @click="showEditDialog = false" class="btn">Cancel</button>
          <button @click="saveEdit" class="btn primary" :disabled="savingEdit || (editWritesSchedule && !!editCronError)">
            {{ savingEdit ? 'Saving...' : 'Save' }}
          </button>
        </div>
      </div>
    </div>

    <!-- Agent Dialog -->
    <div v-if="showAgentDialog" class="overlay" @click.self="showAgentDialog = false">
      <div class="dialog agent-dialog">
        <h3>Create {{ agentTitle }}</h3>
        <label>Target directory</label>
        <div class="dir-picker">
          <select v-model="agentBaseDir" class="mono">
            <option v-for="d in agentDirChoices" :key="d" :value="d">{{ d }}</option>
          </select>
          <button type="button" @click="browseAgentDir" class="btn">Browse…</button>
        </div>
        <div class="hint">
          CronBox writes <span class="mono">cronbox/{{ agentFolder }}/&lt;task&gt;.sh</span> into this directory and runs <span class="mono">{{ agentCommand }}</span> there.
        </div>
        <label>Task name</label>
        <input v-model="agentName" :placeholder="agentKind === 'codex' ? 'daily-codex-task' : 'daily-claude-task'" />
        <label>Prompt</label>
        <textarea
          v-model="agentPrompt"
          class="mono"
          rows="8"
          :placeholder="agentKind === 'codex' ? 'Ask Codex to inspect or maintain this directory...' : 'Ask Claude Code to inspect or maintain this directory...'"
        ></textarea>
        <div class="hint">
          Creates <span class="mono">cronbox/{{ agentFolder }}/&lt;task&gt;.sh</span>, runs it once, then you can schedule it like any script.
        </div>
        <div class="dialog-actions">
          <button @click="showAgentDialog = false" class="btn">Cancel</button>
          <button
            @click="createAgentTask"
            class="btn primary"
            :disabled="creatingAgent || !agentName.trim() || !agentPrompt.trim()"
          >
            {{ creatingAgent ? 'Creating...' : 'Create & Run' }}
          </button>
        </div>
      </div>
    </div>

    <!-- Run Dialog -->
    <div v-if="showRunDialog" class="overlay" @click.self="showRunDialog = false">
      <div class="dialog">
        <h3>Run: {{ runDialogScript?.name }}</h3>

        <div v-if="detectingArgs" class="detecting">Detecting parameters...</div>

        <!-- Form mode: auto-detected params -->
        <template v-else-if="useFormMode && detectedParams.length > 0">
          <div class="form-toggle">
            <button :class="['tab', { active: useFormMode }]" @click="useFormMode = true">Form</button>
            <button :class="['tab', { active: !useFormMode }]" @click="useFormMode = false">JSON</button>
          </div>
          <div class="param-list">
            <div v-for="p in detectedParams" :key="p.name" class="param-row">
              <label class="param-label">
                {{ p.name }}
                <span v-if="p.required" class="req">*</span>
                <span class="param-type">{{ p.param_type }}</span>
              </label>
              <div v-if="p.description" class="param-desc">{{ p.description }}</div>

              <!-- Bool → checkbox -->
              <label v-if="p.param_type === 'bool'" class="param-check">
                <input type="checkbox"
                  :checked="paramValues[p.name] === 'true'"
                  @change="paramValues[p.name] = ($event.target as HTMLInputElement).checked ? 'true' : 'false'" />
                Enable
              </label>

              <!-- Choice → select -->
              <select v-else-if="p.param_type === 'choice'" v-model="paramValues[p.name]" class="param-input">
                <option value="">-- select --</option>
                <option v-for="c in p.choices" :key="c" :value="c">{{ c }}</option>
              </select>

              <!-- Int / Str → input -->
              <input v-else v-model="paramValues[p.name]" class="param-input mono"
                :type="p.param_type === 'int' ? 'number' : 'text'"
                :placeholder="p.default ?? ''" />
            </div>
          </div>
        </template>

        <!-- JSON mode -->
        <template v-else>
          <div v-if="detectedParams.length > 0" class="form-toggle">
            <button :class="['tab', { active: useFormMode }]" @click="useFormMode = true">Form</button>
            <button :class="['tab', { active: !useFormMode }]" @click="useFormMode = false">JSON</button>
          </div>
          <label>Arguments (JSON)</label>
          <textarea v-model="runArgs" class="mono" rows="6" placeholder='{"key": "value"}'></textarea>
        </template>

        <div class="hint">Args passed as CRONBOX_ARGS env var and individual ARG_* vars</div>
        <div class="dialog-actions">
          <button @click="showRunDialog = false" class="btn">Cancel</button>
          <button @click="confirmRun" class="btn primary">Run</button>
        </div>
      </div>
    </div>
  </div>
</template>

<style scoped>
.scripts-page { display: flex; flex-direction: column; height: calc(100vh - 48px); min-width: 0; }
.header { display: flex; align-items: center; gap: 12px; margin-bottom: 16px; }
.header h2 { flex: 1; font-size: 20px; margin: 0; letter-spacing: 0; }
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
.hint { font-size: 12px; margin-top: 8px; color: var(--text-secondary); }

.task-section + .task-section { margin-top: 16px; }
.task-section-head {
  display: flex;
  align-items: center;
  gap: 6px;
  font-size: 11px;
  font-weight: 650;
  color: var(--text-secondary);
  text-transform: uppercase;
  letter-spacing: 0.5px;
  padding: 0 2px;
  margin: 0 0 8px;
}
.task-section-count {
  background: var(--bg-soft);
  border: 1px solid var(--border);
  border-radius: 999px;
  padding: 1px 7px;
  font-size: 10px;
  font-weight: 650;
}

.list { display: flex; flex-direction: column; gap: 5px; }
.row {
  display: flex;
  align-items: center;
  gap: 10px;
  padding: 9px 10px;
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
.row.active {
  border-color: var(--accent-strong);
  background:
    linear-gradient(135deg, var(--accent-soft), rgba(52, 199, 89, 0.08)),
    var(--bg-elevated);
}
.info { flex: 1; min-width: 0; }
.script-link {
  color: inherit;
  text-decoration: none;
  border-radius: 7px;
}
.script-link:hover .name { color: var(--accent); }
.name { font-weight: 600; font-size: 13px; }
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
  margin-top: 2px;
}

.schedule-badge {
  display: flex;
  align-items: center;
  gap: 4px;
  flex-shrink: 0;
  font-size: 11px;
  font-family: monospace;
  color: var(--text-secondary);
  background: var(--bg-soft);
  border: 1px solid var(--border);
  padding: 3px 7px;
  border-radius: 999px;
  max-width: 230px;
  overflow: hidden;
  white-space: nowrap;
}
.dot { width: 6px; height: 6px; border-radius: 50%; }
.dot.on { background: var(--success); box-shadow: 0 0 0 3px rgba(52, 199, 89, 0.12); }
.dot.off { background: var(--text-secondary); }
.next { color: var(--warning); }
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
.inline-check {
  display: flex !important;
  align-items: center;
  gap: 6px;
  margin: 12px 0 4px !important;
  font-size: 12px;
  color: var(--text-secondary);
  cursor: pointer;
  user-select: none;
}
.inline-check input {
  width: auto !important;
  margin: 0;
  accent-color: var(--accent);
}

.btn {
  padding: 5px 10px;
  border: 1px solid var(--border);
  border-radius: 8px;
  background: var(--bg-soft);
  cursor: pointer;
  color: var(--text);
  font-size: 13px;
  box-shadow: var(--glass-highlight);
}
.btn:not(.primary):hover { background: var(--bg-elevated); border-color: var(--border-strong); }
.btn:disabled { opacity: 0.4; cursor: default; box-shadow: none; }
.btn.run { color: var(--success); font-size: 16px; min-width: 34px; }
.btn.quick-run { color: var(--warning); font-size: 14px; min-width: 34px; }
.btn.agent-btn { color: var(--accent); font-weight: 650; }
.btn.claude-btn { color: #8b5cf6; }
.btn.config-btn {
  color: var(--text-secondary);
  font-size: 14px;
  min-width: 30px;
  padding: 4px 8px;
}
.btn.config-btn:hover { color: var(--accent); }
.btn.primary {
  background: linear-gradient(135deg, var(--accent), #49a4ff);
  color: #fff;
  border-color: transparent;
  box-shadow: 0 10px 26px rgba(22, 119, 255, 0.24);
}
.btn.primary:hover {
  background: linear-gradient(135deg, #0f6fe8, #3d98f5);
  color: #fff;
  border-color: transparent;
  box-shadow: 0 12px 30px rgba(22, 119, 255, 0.32);
}
.btn.primary:disabled { opacity: 0.5; }
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

.run-panel {
  margin-top: 12px;
  background: var(--bg-card);
  border: 1px solid var(--border);
  border-radius: 8px;
  overflow: hidden;
  flex: 1;
  min-height: 150px;
  display: flex;
  flex-direction: column;
}
.run-panel-header {
  display: flex;
  align-items: center;
  gap: 8px;
  padding: 9px 12px;
  border-bottom: 1px solid var(--border);
  background: var(--bg-soft);
  font-size: 13px;
}
.run-status { font-size: 16px; }
.run-script { font-weight: 600; overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
.run-badge { font-size: 10px; font-weight: 600; padding: 2px 8px; border-radius: 10px; text-transform: uppercase; }
.st-queued { background: rgba(134,134,139,0.15); color: var(--text-secondary); }
.st-running { background: rgba(0,113,227,0.15); color: var(--accent); }
.st-success { background: rgba(52,199,89,0.15); color: var(--success); }
.st-failure { background: rgba(255,59,48,0.15); color: var(--danger); }
.st-cancelled { background: rgba(255,149,0,0.15); color: var(--warning); }
.run-duration { font-size: 11px; color: var(--text-secondary); font-family: monospace; }
.run-error { padding: 8px 12px; background: rgba(255,59,48,0.08); color: var(--danger); font-size: 12px; font-family: monospace; border-bottom: 1px solid var(--border); }
.run-logs { flex: 1; padding: 10px 12px; font-family: monospace; font-size: 12px; line-height: 1.6; white-space: pre-wrap; word-break: break-all; overflow-y: auto; min-height: 80px; max-height: 400px; }
.log-waiting { color: var(--text-secondary); font-style: italic; }
.log-empty { color: var(--text-secondary); }
.run-result { padding: 6px 12px; border-top: 1px solid var(--border); font-size: 11px; font-family: monospace; background: var(--bg-soft); color: var(--text-secondary); }
.result-label { font-weight: 600; }

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
  background: var(--bg-elevated);
  border: 1px solid var(--border-strong);
  border-radius: 12px;
  padding: 24px;
  width: 420px;
  max-height: 80vh;
  overflow-y: auto;
  box-shadow: var(--glass-shadow), var(--glass-highlight);
  backdrop-filter: var(--glass-blur);
  -webkit-backdrop-filter: var(--glass-blur);
}
.agent-dialog { width: 520px; }
.edit-dialog { width: 520px; }
.edit-target {
  display: flex;
  align-items: center;
  gap: 10px;
  background: var(--bg-soft);
  border: 1px solid var(--border);
  border-radius: 8px;
  padding: 10px;
}
.edit-target-text { min-width: 0; }
.edit-file { font-size: 13px; font-weight: 650; }
.edit-path {
  margin-top: 2px;
  color: var(--text-secondary);
  font-family: monospace;
  font-size: 11px;
  overflow-wrap: anywhere;
}
.schedule-edit-head {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: 16px;
  margin-top: 18px;
  padding-top: 16px;
  border-top: 1px solid var(--border);
}
.section-title {
  font-size: 13px;
  font-weight: 650;
}
.hint.compact { margin-top: 2px; }
.switch {
  display: inline-flex !important;
  align-items: center;
  margin: 0 !important;
  cursor: pointer;
}
.switch input {
  position: absolute;
  opacity: 0;
  pointer-events: none;
}
.switch-track {
  width: 38px;
  height: 22px;
  border-radius: 999px;
  border: 1px solid var(--border);
  background: var(--bg-soft);
  position: relative;
  transition: background 0.16s ease, border-color 0.16s ease;
  box-shadow: var(--glass-highlight);
}
.switch-track::after {
  content: "";
  position: absolute;
  width: 16px;
  height: 16px;
  left: 2px;
  top: 2px;
  border-radius: 50%;
  background: var(--text-secondary);
  transition: transform 0.16s ease, background 0.16s ease;
}
.switch input:checked + .switch-track {
  background: var(--accent-soft);
  border-color: var(--accent-strong);
}
.switch input:checked + .switch-track::after {
  transform: translateX(16px);
  background: var(--accent);
}
.schedule-form.disabled {
  opacity: 0.58;
}
.dir-picker {
  display: flex;
  gap: 8px;
  align-items: center;
}
.dir-picker select {
  flex: 1;
  min-width: 0;
}
.dir-picker .btn {
  white-space: nowrap;
  flex-shrink: 0;
}
.dialog h3 { margin: 0 0 16px; font-size: 16px; }
.dialog label { display: block; font-size: 12px; color: var(--text-secondary); margin: 12px 0 4px; }
.dialog input, .dialog textarea, .dialog select {
  width: 100%;
  padding: 8px;
  border: 1px solid var(--border);
  border-radius: 8px;
  background: var(--bg-soft);
  color: var(--text);
  font-size: 13px;
}
.dialog input:focus, .dialog textarea:focus, .dialog select:focus, .param-input:focus {
  outline: none;
  border-color: var(--accent-strong);
  box-shadow: 0 0 0 3px var(--accent-soft);
}
.mono { font-family: monospace; }
.error { color: var(--danger); font-size: 12px; margin-top: 4px; }
.upcoming { margin-top: 12px; background: var(--bg-soft); border: 1px solid var(--border); border-radius: 8px; padding: 8px 10px; }
.upcoming-title { font-size: 12px; color: var(--text-secondary); margin-bottom: 4px; }
.upcoming-time { font-size: 11px; font-family: monospace; color: var(--text-secondary); }
.dialog-actions { display: flex; justify-content: flex-end; gap: 8px; margin-top: 20px; }

/* Param form */
.detecting { color: var(--text-secondary); font-style: italic; padding: 12px 0; }
.form-toggle {
  display: flex;
  gap: 4px;
  margin-bottom: 12px;
  background: var(--bg-soft);
  border: 1px solid var(--border);
  border-radius: 8px;
  padding: 3px;
}
.tab { padding: 4px 12px; border: 1px solid transparent; border-radius: 6px; background: transparent; cursor: pointer; font-size: 12px; color: var(--text-secondary); }
.tab.active { background: var(--bg-elevated); color: var(--text); border-color: var(--border); box-shadow: var(--glass-highlight); }
.param-list { display: flex; flex-direction: column; gap: 10px; max-height: 350px; overflow-y: auto; }
.param-row { background: var(--bg-soft); border: 1px solid var(--border); border-radius: 8px; padding: 8px; }
.param-label { font-size: 13px; font-weight: 600; display: flex; align-items: center; gap: 4px; margin-bottom: 3px; }
.req { color: var(--danger); }
.param-type { font-size: 10px; color: var(--text-secondary); font-weight: 400; background: var(--bg-code); padding: 1px 5px; border-radius: 6px; }
.param-desc { font-size: 11px; color: var(--text-secondary); margin-bottom: 4px; }
.param-input { width: 100%; padding: 6px 8px; border: 1px solid var(--border); border-radius: 8px; background: var(--bg-soft); color: var(--text); font-size: 13px; }
.param-check { font-size: 13px; display: flex; align-items: center; gap: 6px; }
@media (prefers-color-scheme: dark) {
  .row {
    background:
      linear-gradient(180deg, rgba(255, 255, 255, 0.08), rgba(255, 255, 255, 0.02)),
      var(--bg-card);
  }
}
</style>
