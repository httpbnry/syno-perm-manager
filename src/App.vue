<script setup lang="ts">
import { computed, onMounted, watch } from 'vue'
import { useRoute, useRouter } from 'vue-router'
import { invoke } from '@tauri-apps/api/core'
import { useConnectionStore } from './stores/connection'

const route = useRoute()
const router = useRouter()
const connStore = useConnectionStore()

const currentTheme = computed(() => route.meta.theme as string || 'dark')

onMounted(async () => {
  try {
    const config = await invoke<{ theme: string }>('get_config')
    applyTheme(config.theme)
  } catch {
    applyTheme('dark')
  }
})

function applyTheme(theme: string) {
  document.documentElement.setAttribute('data-theme', theme)
}

watch(currentTheme, (newTheme) => {
  if (newTheme) applyTheme(newTheme)
})

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
      { name: 'settings', label: 'Configuracion', always: true },
    ],
  },
  {
    label: 'General',
    items: [
      { name: 'dashboard', label: 'Dashboard', always: false },
      { name: 'explorer', label: 'Explorador', always: false },
      { name: 'matrix', label: 'Matriz de permisos', always: false },
    ],
  },
  {
    label: 'Gestion',
    items: [
      { name: 'wizard', label: 'Alta de usuario', always: false },
      { name: 'users', label: 'Usuarios y grupos', always: false },
      { name: 'acl-editor', label: 'Editor ACL', always: false },
    ],
  },
  {
    label: 'Auditoria',
    items: [
      { name: 'logs', label: 'Logs', always: false },
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
    <div class="sidebar-header">Syno Perm Manager</div>
    <nav class="sidebar-nav">
      <div v-for="cat in navCategories" :key="cat.label" class="nav-category">
        <div class="nav-category-label">{{ cat.label }}</div>
        <div
          v-for="item in cat.items"
          :key="item.name"
          class="nav-item"
          :class="{
            active: currentRoute === item.name,
            disabled: !canNavigate(item),
          }"
          @click="canNavigate(item) && navigate(item.name)"
        >
          <span>{{ item.label }}</span>
        </div>
      </div>
    </nav>
    <div class="sidebar-footer">
      <span class="status-dot" :class="connStore.isConnected ? 'connected' : 'disconnected'"></span>
      <span v-if="connStore.isConnected">{{ connStore.connectedName }}</span>
      <span v-else>Sin conexion</span>
    </div>
  </div>
  <div class="main-content">
    <router-view />
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
