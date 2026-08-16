<script setup lang="ts">
import { onMounted, onUnmounted, ref } from "vue";
import { useRoute } from "vue-router";
import { api } from "../lib/api";
const route = useRoute();

const inspector = ref(false);
let modeTimer: number | null = null;

async function refreshSchedulerMode() {
  try {
    inspector.value = !(await api.schedulerMode());
  } catch {
    inspector.value = false;
  }
}

onMounted(() => {
  refreshSchedulerMode();
  modeTimer = window.setInterval(refreshSchedulerMode, 2000);
});
onUnmounted(() => {
  if (modeTimer !== null) window.clearInterval(modeTimer);
});

const tabs = [
  { path: "/", label: "Home" },
  { path: "/scripts", label: "Scripts" },
  { path: "/schedules", label: "Schedules" },
  { path: "/history", label: "History" },
  { path: "/settings", label: "Settings" },
];
</script>

<template>
  <nav class="main-nav">
    <router-link to="/" class="brand-link">
      <span class="brand-mark">C</span>
      <span class="brand-copy">
        <span class="brand-text">CronBox</span>
        <span class="brand-subtext">Local Scheduler</span>
      </span>
    </router-link>

    <div class="nav-links">
      <router-link
        v-for="tab in tabs"
        :key="tab.path"
        :to="tab.path"
        class="nav-item"
        :class="{ active: route.path === tab.path }"
        >{{ tab.label }}</router-link
      >
    </div>

    <div
      v-if="inspector"
      class="mode-chip"
      title="Another CronBox process is running the schedules. This server is in standby mode."
    >
      <span class="dot" />
      Standby
    </div>
  </nav>
</template>

<style scoped>
.main-nav {
  display: flex;
  align-items: center;
  flex: 1 1 auto;
  min-width: 0;
  gap: 20px;
}

.brand-link {
  display: flex;
  align-items: center;
  gap: 11px;
  text-decoration: none;
  flex: 0 0 auto;
  -webkit-app-region: no-drag;
}
.brand-mark {
  width: 40px;
  height: 40px;
  display: grid;
  place-items: center;
  border-radius: 12px;
  color: #fff;
  font-size: 20px;
  font-weight: 780;
  letter-spacing: -0.04em;
  background: linear-gradient(180deg, #1488ff, var(--accent));
  box-shadow: 0 12px 28px rgba(0, 113, 227, 0.28), inset 0 1px 0 rgba(255, 255, 255, 0.28);
}
.brand-copy {
  display: flex;
  flex-direction: column;
  gap: 2px;
}
.brand-text {
  font-size: 18px;
  line-height: 1;
  font-weight: 760;
  letter-spacing: -0.04em;
  color: var(--text);
  white-space: nowrap;
}
.brand-subtext {
  color: var(--muted-2);
  font-size: 11px;
  line-height: 1.2;
  letter-spacing: -0.01em;
  white-space: nowrap;
}

.nav-links {
  display: inline-flex;
  align-items: center;
  gap: 4px;
  padding: 5px;
  border: 1px solid var(--border);
  border-radius: 999px;
  background: rgba(255, 255, 255, 0.62);
  box-shadow: inset 0 1px 0 rgba(255, 255, 255, 0.78);
  backdrop-filter: saturate(180%) blur(18px);
  -webkit-backdrop-filter: saturate(180%) blur(18px);
  -webkit-app-region: no-drag;
}
.nav-item {
  padding: 8px 15px;
  border-radius: 999px;
  text-decoration: none;
  color: #424245;
  font-weight: 650;
  font-size: 13px;
  letter-spacing: -0.01em;
  transition: color 0.16s ease, background 0.16s ease, box-shadow 0.16s ease, transform 0.16s ease;
}
.nav-item:hover {
  color: var(--accent);
  background: var(--accent-soft);
}
.nav-item.active {
  color: #fff;
  background: linear-gradient(180deg, #1488ff, var(--accent));
  box-shadow: 0 8px 18px rgba(0, 113, 227, 0.22), inset 0 1px 0 rgba(255, 255, 255, 0.26);
}

.mode-chip {
  margin-left: auto;
  display: inline-flex;
  align-items: center;
  gap: 6px;
  padding: 5px 11px;
  border-radius: 999px;
  font-size: 11px;
  font-weight: 650;
  color: var(--warning);
  background: rgba(255, 149, 0, 0.12);
  border: 1px solid rgba(255, 149, 0, 0.34);
  -webkit-app-region: no-drag;
  cursor: default;
}
.mode-chip .dot {
  width: 6px;
  height: 6px;
  border-radius: 999px;
  background: var(--warning);
}

@media (max-width: 820px) {
  .main-nav {
    flex-wrap: wrap;
    gap: 12px;
  }
  .nav-links {
    flex-wrap: wrap;
  }
}
</style>
