import { createApp } from "vue";
import { createRouter, createWebHistory } from "vue-router";
import "./assets/main.css";
import App from "./App.vue";
import DashboardView from "./views/DashboardView.vue";
import ScriptsView from "./views/ScriptsView.vue";
import ScriptDetailView from "./views/ScriptDetailView.vue";
import SchedulesView from "./views/SchedulesView.vue";
import HistoryView from "./views/HistoryView.vue";
import SettingsView from "./views/SettingsView.vue";
import LoginView from "./views/LoginView.vue";
import { getAuthToken } from "./lib/api";

const router = createRouter({
  history: createWebHistory(),
  routes: [
    { path: "/", component: DashboardView },
    { path: "/scripts", component: ScriptsView },
    { path: "/scripts/detail", component: ScriptDetailView },
    { path: "/schedules", component: SchedulesView },
    { path: "/history", component: HistoryView },
    { path: "/settings", component: SettingsView },
    { path: "/login", component: LoginView },
  ],
});

router.beforeEach((to) => {
  if (to.path !== "/login" && !getAuthToken()) {
    return "/login";
  }
  return true;
});

createApp(App).use(router).mount("#app");
