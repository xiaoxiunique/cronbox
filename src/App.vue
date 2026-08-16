<script setup lang="ts">
import { computed } from "vue";
import { useRoute } from "vue-router";
import TopNav from "./components/TopNav.vue";

const route = useRoute();
const isLoginPage = computed(() => route.path === "/login");
</script>

<template>
  <div class="app">
    <header v-if="!isLoginPage" class="topbar">
      <div class="topbar-inner page-container">
        <TopNav />
      </div>
    </header>
    <main class="main">
      <div class="page-container">
        <router-view />
      </div>
    </main>
  </div>
</template>
