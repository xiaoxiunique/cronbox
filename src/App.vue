<script setup lang="ts">
import Sidebar from "./components/Sidebar.vue";
</script>

<template>
  <div class="app">
    <Sidebar />
    <main class="main">
      <router-view />
    </main>
  </div>
</template>

<style>
:root {
  --sidebar-width: 180px;
  --bg: #edf2f7;
  --bg-card: rgba(255, 255, 255, 0.66);
  --bg-elevated: rgba(255, 255, 255, 0.84);
  --bg-soft: rgba(255, 255, 255, 0.46);
  --bg-code: rgba(18, 29, 43, 0.05);
  --border: rgba(45, 70, 100, 0.14);
  --border-strong: rgba(45, 70, 100, 0.22);
  --text: #1d1d1f;
  --text-secondary: #69717c;
  --accent: #1677ff;
  --accent-soft: rgba(22, 119, 255, 0.12);
  --accent-strong: rgba(22, 119, 255, 0.34);
  --success: #34c759;
  --danger: #ff3b30;
  --warning: #ff9500;
  --glass-blur: blur(22px) saturate(1.35);
  --glass-shadow: 0 18px 45px rgba(31, 54, 82, 0.12);
  --glass-shadow-soft: 0 10px 28px rgba(31, 54, 82, 0.08);
  --glass-highlight: inset 0 1px 0 rgba(255, 255, 255, 0.64);
  --scrollbar-size: 10px;
  --scrollbar-track: rgba(255, 255, 255, 0.22);
  --scrollbar-thumb: rgba(68, 94, 126, 0.26);
  --scrollbar-thumb-hover: rgba(22, 119, 255, 0.46);
}
@media (prefers-color-scheme: dark) {
  :root {
    --bg: #111418;
    --bg-card: rgba(35, 37, 42, 0.62);
    --bg-elevated: rgba(44, 47, 54, 0.78);
    --bg-soft: rgba(255, 255, 255, 0.06);
    --bg-code: rgba(255, 255, 255, 0.06);
    --border: rgba(255, 255, 255, 0.10);
    --border-strong: rgba(255, 255, 255, 0.16);
    --text: #f5f5f7;
    --text-secondary: #a1a6af;
    --accent: #5da2ff;
    --accent-soft: rgba(93, 162, 255, 0.16);
    --accent-strong: rgba(93, 162, 255, 0.32);
    --glass-shadow: 0 18px 48px rgba(0, 0, 0, 0.38);
    --glass-shadow-soft: 0 10px 28px rgba(0, 0, 0, 0.22);
    --glass-highlight: inset 0 1px 0 rgba(255, 255, 255, 0.10);
    --scrollbar-track: rgba(255, 255, 255, 0.04);
    --scrollbar-thumb: rgba(255, 255, 255, 0.22);
    --scrollbar-thumb-hover: rgba(93, 162, 255, 0.56);
  }
}
* { box-sizing: border-box; }
html, body, #app { height: 100%; margin: 0; }
body {
  background:
    linear-gradient(180deg, rgba(255, 255, 255, 0.76) 0%, rgba(255, 255, 255, 0) 36%),
    linear-gradient(135deg, #f8fafc 0%, var(--bg) 46%, #e5ecf3 100%);
  color: var(--text);
  font-family: -apple-system, BlinkMacSystemFont, "SF Pro Text", "Segoe UI", sans-serif;
  overflow: hidden;
}
@media (prefers-color-scheme: dark) {
  body {
    background:
      linear-gradient(180deg, rgba(255, 255, 255, 0.06) 0%, rgba(255, 255, 255, 0) 34%),
      linear-gradient(135deg, #0c0e11 0%, var(--bg) 48%, #191d23 100%);
  }
}
button, input, textarea, select { font: inherit; }
button { color: inherit; }
* {
  scrollbar-color: var(--scrollbar-thumb) transparent;
  scrollbar-width: thin;
}
*::-webkit-scrollbar {
  width: var(--scrollbar-size);
  height: var(--scrollbar-size);
}
*::-webkit-scrollbar-track {
  background: transparent;
}
*::-webkit-scrollbar-thumb {
  min-height: 42px;
  border: 3px solid transparent;
  border-radius: 999px;
  background:
    linear-gradient(var(--scrollbar-thumb), var(--scrollbar-thumb)) padding-box,
    var(--scrollbar-track) border-box;
  box-shadow: inset 0 0 0 1px rgba(255, 255, 255, 0.14);
}
*::-webkit-scrollbar-thumb:hover {
  background:
    linear-gradient(var(--scrollbar-thumb-hover), var(--scrollbar-thumb-hover)) padding-box,
    var(--scrollbar-track) border-box;
}
*::-webkit-scrollbar-corner {
  background: transparent;
}
.app {
  display: flex;
  height: 100vh;
  position: relative;
  isolation: isolate;
}
.app::before {
  content: "";
  position: fixed;
  inset: 0;
  pointer-events: none;
  background-image:
    linear-gradient(rgba(255,255,255,0.20) 1px, transparent 1px),
    linear-gradient(90deg, rgba(255,255,255,0.16) 1px, transparent 1px);
  background-size: 42px 42px;
  mask-image: linear-gradient(to bottom, rgba(0,0,0,0.7), transparent 75%);
  opacity: 0.28;
  z-index: -1;
}
.main {
  flex: 1;
  overflow-y: auto;
  padding: 24px;
  scrollbar-gutter: stable;
}
.glass-card,
.row,
.card,
.detail,
.run-panel,
.dialog,
.job-list {
  background: var(--bg-card);
  border: 1px solid var(--border);
  backdrop-filter: var(--glass-blur);
  -webkit-backdrop-filter: var(--glass-blur);
  box-shadow: var(--glass-shadow-soft), var(--glass-highlight);
}
.btn,
.btn-icon,
button {
  transition: background 0.16s ease, border-color 0.16s ease, transform 0.16s ease, box-shadow 0.16s ease;
}
.btn:hover,
.btn-icon:hover {
  transform: translateY(-1px);
}
.btn:disabled,
.btn-icon:disabled {
  transform: none;
}
input,
textarea,
select {
  backdrop-filter: blur(14px);
  -webkit-backdrop-filter: blur(14px);
}
::selection { background: var(--accent-soft); }
</style>
