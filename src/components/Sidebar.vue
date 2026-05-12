<script setup lang="ts">
import { useRoute } from "vue-router";
const route = useRoute();

const tabs = [
  { path: "/", label: "Home", icon: "⌂" },
  { path: "/scripts", label: "Scripts", icon: "📄" },
  { path: "/schedules", label: "Schedules", icon: "⏰" },
  { path: "/history", label: "History", icon: "📋" },
  { path: "/settings", label: "Settings", icon: "⚙️" },
];
</script>

<template>
  <nav class="sidebar">
    <div class="logo">CronBox</div>
    <router-link
      v-for="tab in tabs"
      :key="tab.path"
      :to="tab.path"
      :class="['nav-item', { active: route.path === tab.path }]"
    >
      <span class="icon">{{ tab.icon }}</span>
      <span>{{ tab.label }}</span>
    </router-link>
  </nav>
</template>

<style scoped>
.sidebar {
  width: var(--sidebar-width);
  background:
    linear-gradient(180deg, rgba(255, 255, 255, 0.52), rgba(255, 255, 255, 0.24)),
    var(--bg-card);
  border-right: 1px solid var(--border);
  display: flex;
  flex-direction: column;
  padding: 14px 10px;
  gap: 4px;
  -webkit-app-region: drag;
  user-select: none;
  backdrop-filter: var(--glass-blur);
  -webkit-backdrop-filter: var(--glass-blur);
  box-shadow: var(--glass-highlight);
}
.logo {
  font-size: 18px;
  font-weight: 760;
  letter-spacing: 0;
  padding: 12px 12px 18px;
  color: var(--text);
  text-shadow: 0 1px 0 rgba(255, 255, 255, 0.55);
}
.nav-item {
  display: flex;
  align-items: center;
  gap: 8px;
  padding: 9px 11px;
  border: 1px solid transparent;
  border-radius: 8px;
  text-decoration: none;
  color: var(--text-secondary);
  font-size: 13px;
  -webkit-app-region: no-drag;
  transition: background 0.16s ease, color 0.16s ease, border-color 0.16s ease, box-shadow 0.16s ease, transform 0.16s ease;
}
.nav-item:hover {
  background: var(--bg-soft);
  border-color: var(--border);
  color: var(--text);
}
.nav-item.active {
  background:
    linear-gradient(135deg, rgba(22, 119, 255, 0.20), rgba(52, 199, 89, 0.08)),
    var(--bg-elevated);
  border-color: var(--accent-strong);
  color: var(--text);
  box-shadow: 0 8px 24px rgba(22, 119, 255, 0.14), var(--glass-highlight);
}
.icon { font-size: 16px; }
@media (prefers-color-scheme: dark) {
  .sidebar {
    background:
      linear-gradient(180deg, rgba(255, 255, 255, 0.08), rgba(255, 255, 255, 0.02)),
      var(--bg-card);
  }
  .logo { text-shadow: none; }
}
</style>
