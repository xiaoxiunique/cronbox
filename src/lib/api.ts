import { invoke } from "@tauri-apps/api/core";

export interface WorkDir {
  id: string;
  path: string;
  created_at: string;
}

export interface AddedWorkDir {
  work_dir: WorkDir;
  entry_scripts: ScriptFile[];
}

export interface CreatedCodexTask {
  script: ScriptFile;
  full_path: string;
}

export interface ScriptFile {
  path: string;
  name: string;
  alias: string;
  language: string;
  base_dir: string;
}

export interface Schedule {
  id: string;
  script_path: string;
  base_dir: string;
  cron_expr: string;
  timezone: string;
  args: string;
  enabled: boolean;
  next_run_at: string | null;
  last_run_at: string | null;
  created_at: string;
  updated_at: string;
}

export interface Job {
  id: string;
  schedule_id: string | null;
  script_path: string;
  base_dir: string;
  status: string;
  args: string;
  result: string | null;
  logs: string;
  error: string | null;
  scheduled_for: string | null;
  started_at: string | null;
  completed_at: string | null;
  duration_ms: number | null;
  created_at: string;
}

export interface RecentRunStats {
  total: number;
  success: number;
  failure: number;
  running: number;
  queued: number;
  cancelled: number;
  skipped: number;
}

export interface ScheduleDistributionBucket {
  hour: string;
  count: number;
}

export interface DashboardStats {
  script_total: number;
  schedule_total: number;
  enabled_schedule_total: number;
  recent_runs: RecentRunStats;
  schedule_distribution: ScheduleDistributionBucket[];
}

export interface ScriptParam {
  name: string;
  param_type: string;
  default: string | null;
  required: boolean;
  description: string;
  choices: string[];
}

export const api = {
  dashboardStats: () => invoke<DashboardStats>("dashboard_stats"),

  // Work dirs
  listWorkDirs: () => invoke<WorkDir[]>("list_work_dirs"),
  addWorkDir: (path: string) => invoke<WorkDir>("add_work_dir", { path }),
  addWorkDirWithScan: (path: string) => invoke<AddedWorkDir>("add_work_dir_with_scan", { path }),
  removeWorkDir: (id: string) => invoke<boolean>("remove_work_dir", { id }),

  scanScripts: () => invoke<ScriptFile[]>("scan_scripts"),
  setScriptAlias: (baseDir: string, scriptPath: string, alias?: string) =>
    invoke<void>("set_script_alias", { baseDir, scriptPath, alias }),
  ensureAgentWorkspace: () => invoke<WorkDir>("ensure_agent_workspace"),
  createCodexTask: (name: string, prompt: string) =>
    invoke<CreatedCodexTask>("create_codex_task", { name, prompt }),
  createClaudeTask: (name: string, prompt: string) =>
    invoke<CreatedCodexTask>("create_claude_task", { name, prompt }),

  createSchedule: (scriptPath: string, baseDir: string, cronExpr: string, timezone: string, args: string) =>
    invoke<Schedule>("create_schedule", { scriptPath, baseDir, cronExpr, timezone, args }),
  listSchedules: () => invoke<Schedule[]>("list_schedules"),
  updateSchedule: (id: string, cronExpr?: string, timezone?: string, args?: string) =>
    invoke<Schedule>("update_schedule", { id, cronExpr, timezone, args }),
  setScheduleEnabled: (id: string, enabled: boolean) =>
    invoke<void>("set_schedule_enabled", { id, enabled }),
  deleteSchedule: (id: string) => invoke<boolean>("delete_schedule", { id }),

  runNow: (scriptPath: string, baseDir: string, args: string = "{}") =>
    invoke<Job>("run_now", { scriptPath, baseDir, args }),
  listJobs: (limit: number = 100) => invoke<Job[]>("list_jobs", { limit }),
  listJobsForScript: (scriptPath: string, baseDir: string, limit: number = 100) =>
    invoke<Job[]>("list_jobs_for_script", { scriptPath, baseDir, limit }),
  listRunningJobs: () => invoke<Job[]>("list_running_jobs"),
  getJob: (id: string) => invoke<Job>("get_job", { id }),
  cancelJob: (id: string) => invoke<boolean>("cancel_job", { id }),
  cleanupOldJobs: (days: number = 30) => invoke<number>("cleanup_old_jobs", { days }),
  cliStatus: () => invoke<string>("cli_status"),
  installCli: (force: boolean = false) => invoke<string>("install_cli", { force }),

  validateCron: (cronExpr: string) => invoke<void>("validate_cron", { cronExpr }),
  upcomingRuns: (cronExpr: string, timezone: string, count: number = 5) =>
    invoke<string[]>("upcoming_runs", { cronExpr, timezone, count }),
  detectArgs: (scriptPath: string, baseDir: string) =>
    invoke<ScriptParam[]>("detect_args", { scriptPath, baseDir }),
};
