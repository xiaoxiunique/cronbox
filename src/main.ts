import { createApp } from "vue";
import { createRouter, createWebHistory } from "vue-router";
import App from "./App.vue";
import ScriptsView from "./views/ScriptsView.vue";
import ScriptDetailView from "./views/ScriptDetailView.vue";
import SchedulesView from "./views/SchedulesView.vue";
import HistoryView from "./views/HistoryView.vue";
import SettingsView from "./views/SettingsView.vue";

const router = createRouter({
  history: createWebHistory(),
  routes: [
    { path: "/", redirect: "/scripts" },
    { path: "/scripts", component: ScriptsView },
    { path: "/scripts/detail", component: ScriptDetailView },
    { path: "/schedules", component: SchedulesView },
    { path: "/history", component: HistoryView },
    { path: "/settings", component: SettingsView },
  ],
});

createApp(App).use(router).mount("#app");
