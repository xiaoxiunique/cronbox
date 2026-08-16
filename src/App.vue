<script setup lang="ts">
import TopNav from "./components/TopNav.vue";
</script>

<template>
  <div class="app">
    <header class="topbar">
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
