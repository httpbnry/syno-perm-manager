<script setup lang="ts">
import { computed, onMounted, ref } from 'vue'
import { useRoute, useRouter } from 'vue-router'
import { invoke } from '@tauri-apps/api/core'
import { useConnectionStore } from './stores/connection'

const route = useRoute()
const router = useRouter()
const connStore = useConnectionStore()

const currentTheme = ref('dark')

onMounted(async () => {
  try {
    const config = await invoke<{ theme: string }>('get_config')
    applyTheme(config.theme)
  } catch {
    applyTheme('dark')
  }
})

function applyTheme(theme: string) {
  currentTheme.value = theme === 'light' ? 'light' : 'dark'
  document.documentElement.setAttribute('data-theme', theme)
}

const themeError = ref('')
async function toggleTheme() {
  const next = document.documentElement.getAttribute('data-theme') === 'light' ? 'dark' : 'light'
  themeError.value = ''
  try {
    const config = await invoke<Record<string, unknown>>('get_config')
    await invoke('save_config', { dto: { ...config, theme: next } })
    applyTheme(next)
  } catch (e) { themeError.value = String(e) }
}

const icons: Record<string, string> = {
  connections: 'M4 4h16v6H4z M4 14h16v6H4z M7 7h.01 M7 17h.01',
  settings: 'M12 8a4 4 0 1 0 0 8 4 4 0 0 0 0-8 M12 2v3 M12 19v3 M2 12h3 M19 12h3 M5 5l2 2 M17 17l2 2 M5 19l2-2 M17 7l2-2',
  dashboard: 'M3 3h7v7H3z M14 3h7v7h-7z M3 14h7v7H3z M14 14h7v7h-7z',
  explorer: 'M3 7V4h6l3 3h9v13H3z',
  matrix: 'M3 3h18v18H3z M3 9h18 M3 15h18 M9 3v18 M15 3v18',
  compare: 'M4 7h16l-4-4 M20 17H4l4 4 M20 7l-4 4 M4 17l4-4',
  wizard: 'M10 3a4 4 0 1 0 0 8 4 4 0 0 0 0-8 M3 21v-3a7 7 0 0 1 12-5 M19 13v8 M15 17h8',
  users: 'M9 3a4 4 0 1 0 0 8 4 4 0 0 0 0-8 M2 21v-3a7 7 0 0 1 14 0v3 M17 4a4 4 0 0 1 0 8 M19 15a5 5 0 0 1 3 6',
  'acl-editor': 'M12 3l8 3v6c0 5-8 9-8 9s-8-4-8-9V6z M8 12l3 3 5-6',
  logs: 'M5 3h14v18H5z M8 7h8 M8 12h8 M8 17h5',
}

interface NavItem {
  name: string
  label: string
  always: boolean
}
interface NavCategory {
  label: string
  items: NavItem[]
}

const navCategories: NavCategory[] = [
  {
    label: 'Sistema',
    items: [
      { name: 'connections', label: 'Conexiones', always: true },
      { name: 'settings', label: 'Configuración', always: true },
    ],
  },
  {
    label: 'General',
    items: [
      { name: 'dashboard', label: 'Vista general', always: false },
      { name: 'explorer', label: 'Explorador', always: false },
      { name: 'matrix', label: 'Matriz de permisos', always: false },
      { name: 'compare', label: 'Comparar permisos', always: false },
    ],
  },
  {
    label: 'Gestión',
    items: [
      { name: 'wizard', label: 'Alta de usuario', always: false },
      { name: 'users', label: 'Usuarios y grupos', always: false },
      { name: 'acl-editor', label: 'Editor ACL', always: false },
    ],
  },
  {
    label: 'Auditoría',
    items: [
      { name: 'logs', label: 'Historial de cambios', always: false },
    ],
  },
]

const currentRoute = computed(() => route.name as string)

function navigate(name: string) {
  router.push({ name })
}

function canNavigate(item: NavItem): boolean {
  return item.always || connStore.isConnected
}
</script>

<template>
  <div class="sidebar">
    <div class="sidebar-header"><span class="brand-mark">S</span><span>Syno<span class="brand-subtitle">PERMISSION MANAGER</span></span></div>
    <nav class="sidebar-nav" aria-label="Navegación principal">
      <div v-for="cat in navCategories" :key="cat.label" class="nav-category">
        <div class="nav-category-label">{{ cat.label }}</div>
        <button
          v-for="item in cat.items"
          :key="item.name"
          class="nav-item"
          :disabled="!canNavigate(item)"
          :aria-current="currentRoute === item.name ? 'page' : undefined"
          :class="{
            active: currentRoute === item.name,
            disabled: !canNavigate(item),
          }"
          @click="canNavigate(item) && navigate(item.name)"
        >
          <svg width="18" height="18" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.6" stroke-linecap="round" stroke-linejoin="round" aria-hidden="true"><path :d="icons[item.name]" /></svg>
          <span>{{ item.label }}</span>
        </button>
      </div>
    </nav>
    <div class="sidebar-footer">
      <span class="status-dot" :class="connStore.isConnected ? 'connected' : 'disconnected'"></span>
      <span v-if="connStore.isConnected">{{ connStore.connectedName }}</span>
      <span v-else>Sin conexión</span>
    </div>
  </div>
  <div class="workspace">
    <header class="workspace-header"><div><span class="workspace-label">ESPACIO DE TRABAJO</span><strong>{{ connStore.isConnected ? connStore.connectedName : 'Administración de Synology' }}</strong></div><div class="workspace-actions"><span class="badge" :class="connStore.isConnected ? 'badge-success' : 'badge-info'">{{ connStore.isConnected ? 'SSH conectado' : 'Desconectado' }}</span><button class="btn btn-secondary btn-sm" @click="toggleTheme" aria-label="Cambiar tema claro u oscuro">◐ Tema</button></div></header>
    <main class="main-content" id="main-content">
      <div v-if="themeError" class="alert alert-error" role="alert">{{ themeError }}</div>
      <router-view />
    </main>
  </div>
</template>

<style scoped>
.nav-category {
  margin-bottom: 8px;
}

.nav-category-label {
  font-size: 10px;
  font-weight: 600;
  color: var(--text-muted);
  text-transform: uppercase;
  letter-spacing: 1px;
  padding: 8px 14px 4px;
}
</style>
