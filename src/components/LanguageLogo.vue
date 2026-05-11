<script setup lang="ts">
const props = defineProps<{
  language: string;
}>();

type LanguageKey = "python" | "bash" | "bun" | "pgsql" | "file";
const languageKeys: Record<string, LanguageKey> = {
  python: "python",
  bash: "bash",
  bun: "bun",
  pgsql: "pgsql",
};

function languageKey(lang: string): LanguageKey {
  return languageKeys[lang] ?? "file";
}

function languageName(lang: string) {
  return {
    python: "Python",
    bash: "Bash",
    bun: "Bun",
    pgsql: "PostgreSQL",
    file: "Script",
  }[languageKey(lang)]!;
}

function languageShort(lang: string) {
  return {
    python: "Py",
    bash: "$",
    bun: "Bun",
    pgsql: "PG",
    file: "SH",
  }[languageKey(lang)]!;
}
</script>

<template>
  <span
    :class="['lang-logo', `lang-${languageKey(props.language)}`]"
    :title="languageName(props.language)"
    :aria-label="languageName(props.language)"
  >
    <svg v-if="languageKey(props.language) === 'python'" viewBox="0 0 32 32" aria-hidden="true">
      <path class="py-blue" d="M15.6 4.2c-4.5 0-6.7 1.4-6.7 4.1v3.3h8.3v1.9H7.1c-2.5 0-4.4 2.1-4.4 4.8 0 2.8 1.7 4.8 4.2 4.8h2.4v-3.4c0-2.7 2.2-4.6 4.9-4.6h6.7c2.2 0 3.9-1.7 3.9-3.9V8.4c0-2.7-2.4-4.2-6.8-4.2h-2.4Z" />
      <path class="py-yellow" d="M16.4 27.8c4.5 0 6.7-1.4 6.7-4.1v-3.3h-8.3v-1.9h10.1c2.5 0 4.4-2.1 4.4-4.8 0-2.8-1.7-4.8-4.2-4.8h-2.4v3.4c0 2.7-2.2 4.6-4.9 4.6h-6.7c-2.2 0-3.9 1.7-3.9 3.9v2.8c0 2.7 2.4 4.2 6.8 4.2h2.4Z" />
      <circle class="py-dot-light" cx="12.1" cy="7.5" r="1.1" />
      <circle class="py-dot-dark" cx="19.9" cy="24.5" r="1.1" />
    </svg>
    <svg v-else-if="languageKey(props.language) === 'bash'" viewBox="0 0 32 32" aria-hidden="true">
      <rect class="bash-screen" x="4" y="6" width="24" height="20" rx="5" />
      <path class="bash-prompt" d="M10 12.4 14.2 16 10 19.6" />
      <path class="bash-cursor" d="M16.5 20h5.2" />
    </svg>
    <svg v-else-if="languageKey(props.language) === 'bun'" viewBox="0 0 32 32" aria-hidden="true">
      <path class="bun-ear" d="M9.3 10.5c-.6-2.1.3-4 1.9-4.4 1.3-.3 2.7.6 3.5 2.2" />
      <path class="bun-ear" d="M22.7 10.5c.6-2.1-.3-4-1.9-4.4-1.3-.3-2.7.6-3.5 2.2" />
      <circle class="bun-face" cx="16" cy="17" r="10.2" />
      <circle class="bun-eye" cx="12.4" cy="16.2" r="1.2" />
      <circle class="bun-eye" cx="19.6" cy="16.2" r="1.2" />
      <path class="bun-mouth" d="M13.5 20.2c1.6 1.2 3.4 1.2 5 0" />
    </svg>
    <svg v-else-if="languageKey(props.language) === 'pgsql'" viewBox="0 0 32 32" aria-hidden="true">
      <ellipse class="pg-top" cx="16" cy="8.5" rx="9.6" ry="4.2" />
      <path class="pg-body" d="M6.4 8.5v12c0 2.3 4.3 4.2 9.6 4.2s9.6-1.9 9.6-4.2v-12" />
      <path class="pg-line" d="M6.4 14.5c0 2.3 4.3 4.2 9.6 4.2s9.6-1.9 9.6-4.2" />
      <text x="16" y="14.1" text-anchor="middle" class="pg-text">PG</text>
    </svg>
    <span v-else class="lang-fallback">{{ languageShort(props.language) }}</span>
  </span>
</template>

<style scoped>
.lang-logo {
  width: 30px;
  height: 30px;
  flex: 0 0 30px;
  display: inline-flex;
  align-items: center;
  justify-content: center;
  border: 1px solid var(--border);
  border-radius: 8px;
  background: var(--bg-soft);
  box-shadow: var(--glass-highlight);
  overflow: hidden;
}
.lang-logo svg {
  width: 24px;
  height: 24px;
  display: block;
}
.lang-python {
  background: linear-gradient(135deg, rgba(55, 118, 171, 0.18), rgba(255, 212, 59, 0.16)), var(--bg-soft);
}
.py-blue { fill: #3776ab; }
.py-yellow { fill: #ffd43b; }
.py-dot-light { fill: #fff6c2; }
.py-dot-dark { fill: #254a6b; }
.lang-bash {
  background: linear-gradient(135deg, rgba(47, 145, 80, 0.18), rgba(18, 29, 43, 0.08)), var(--bg-soft);
}
.bash-screen { fill: #1f2933; }
.bash-prompt,
.bash-cursor {
  fill: none;
  stroke: #4ade80;
  stroke-width: 2.4;
  stroke-linecap: round;
  stroke-linejoin: round;
}
.lang-bun {
  background: linear-gradient(135deg, rgba(251, 204, 121, 0.30), rgba(152, 92, 48, 0.10)), var(--bg-soft);
}
.bun-face { fill: #f6c982; stroke: #9b6438; stroke-width: 1.7; }
.bun-ear { fill: none; stroke: #9b6438; stroke-width: 2; stroke-linecap: round; }
.bun-eye { fill: #3d2418; }
.bun-mouth { fill: none; stroke: #3d2418; stroke-width: 1.5; stroke-linecap: round; }
.lang-pgsql {
  background: linear-gradient(135deg, rgba(49, 99, 140, 0.22), rgba(74, 144, 226, 0.10)), var(--bg-soft);
}
.pg-top,
.pg-body { fill: #336791; }
.pg-body { opacity: 0.94; }
.pg-line { fill: none; stroke: rgba(255,255,255,0.66); stroke-width: 1.4; }
.pg-text {
  fill: #fff;
  font: 700 7px -apple-system, BlinkMacSystemFont, "SF Pro Text", sans-serif;
  letter-spacing: 0;
}
.lang-file {
  background: linear-gradient(135deg, rgba(134, 134, 139, 0.18), rgba(255, 255, 255, 0.08)), var(--bg-soft);
}
.lang-fallback {
  font-size: 10px;
  font-weight: 760;
  color: var(--text-secondary);
  letter-spacing: 0;
}
</style>
