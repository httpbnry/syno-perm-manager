<script setup lang="ts">
import { ref, onMounted, computed } from 'vue'
import { useRouter } from 'vue-router'
import { invoke } from '@tauri-apps/api/core'
import { useConnectionStore } from '../stores/connection'
import type { DashboardStats } from '../types'

const router = useRouter()
const connStore = useConnectionStore()
const stats = ref<DashboardStats | null>(null)
const loading = ref(true)

onMounted(async () => {
  if (!connStore.isConnected) {
    router.push('/connections')
    return
  }
  try {
    stats.value = await invoke<DashboardStats>('get_dashboard_stats')
  } catch (e: any) {
    console.error(e)
  } finally {
    loading.value = false
  }
})

const successRate = computed(() => {
  if (!stats.value || stats.value.total_changes === 0) return 100
  return Math.round((stats.value.successful_changes / stats.value.total_changes) * 100)
})
</script>

<template>
  <div class="view-header">
    <h1>Dashboard</h1>
    <p>Resumen general del NAS {{ connStore.connectedName }}</p>
  </div>

  <div v-if="loading" class="card">
    <div class="empty-state">
      <span class="loading-spinner"></span>
      <p>Cargando estadisticas...</p>
    </div>
  </div>

  <div v-else-if="stats">
    <!-- Stats cards -->
    <div class="stats-grid">
      <div class="stat-card" @click="router.push('/users')">
        <div class="stat-icon">&#128100;</div>
        <div class="stat-value">{{ stats.user_count }}</div>
        <div class="stat-label">Usuarios</div>
      </div>
      <div class="stat-card" @click="router.push('/users')">
        <div class="stat-icon">&#128101;</div>
        <div class="stat-value">{{ stats.group_count }}</div>
        <div class="stat-label">Grupos</div>
      </div>
      <div class="stat-card" @click="router.push('/explorer')">
        <div class="stat-icon">&#128193;</div>
        <div class="stat-value">{{ stats.share_count }}</div>
        <div class="stat-label">Carpetas</div>
      </div>
      <div class="stat-card" @click="router.push('/logs')">
        <div class="stat-icon">&#128221;</div>
        <div class="stat-value">{{ stats.total_changes }}</div>
        <div class="stat-label">Cambios totales</div>
      </div>
    </div>

    <!-- Quick actions -->
    <div class="card">
      <div class="card-title">Acciones rapidas</div>
      <div class="quick-actions">
        <button class="btn btn-primary" @click="router.push('/explorer')">&#128193; Editar permisos</button>
        <button class="btn btn-primary" @click="router.push('/users')">&#128100; Gestionar usuarios</button>
        <button class="btn btn-primary" @click="router.push('/matrix')">&#128202; Matriz de permisos</button>
        <button class="btn btn-primary" @click="router.push('/wizard')">&#10024; Alta de usuario</button>
        <button class="btn btn-secondary" @click="router.push('/logs')">&#128221; Ver logs</button>
      </div>
    </div>

    <!-- Recent activity -->
    <div class="card">
      <div class="card-title">Actividad reciente</div>
      <table v-if="stats.recent_logs.length > 0">
        <thead>
          <tr>
            <th>Fecha</th>
            <th>Accion</th>
            <th>Ruta</th>
            <th>Estado</th>
          </tr>
        </thead>
        <tbody>
          <tr v-for="log in stats.recent_logs" :key="log.id">
            <td style="font-family: monospace; font-size: 11px;">{{ log.timestamp }}</td>
            <td><span class="badge badge-info">{{ log.action }}</span></td>
            <td style="font-family: monospace; font-size: 11px; max-width: 300px; overflow: hidden; text-overflow: ellipsis;">{{ log.path }}</td>
            <td>
              <span v-if="log.success" class="badge badge-success">OK</span>
              <span v-else class="badge badge-danger">Error</span>
            </td>
          </tr>
        </tbody>
      </table>
      <div v-else class="empty-state">
        <p>Sin actividad reciente. Los cambios que hagas aparecen aqui.</p>
      </div>
    </div>

    <!-- Success rate -->
    <div class="card" v-if="stats.total_changes > 0">
      <div class="card-title">Tasa de exito</div>
      <div class="success-bar-container">
        <div class="success-bar" :style="{ width: successRate + '%' }"></div>
      </div>
      <div style="display: flex; justify-content: space-between; margin-top: 8px; font-size: 12px;">
        <span style="color: var(--success);">{{ stats.successful_changes }} exitos</span>
        <span style="color: var(--accent);">{{ successRate }}%</span>
        <span style="color: var(--danger);">{{ stats.failed_changes }} errores</span>
      </div>
    </div>
  </div>
</template>

<style scoped>
.stats-grid {
  display: grid;
  grid-template-columns: repeat(4, 1fr);
  gap: 12px;
  margin-bottom: 16px;
}

.stat-card {
  background: var(--bg-secondary);
  border: 1px solid var(--border);
  border-radius: var(--radius);
  padding: 20px;
  text-align: center;
  cursor: pointer;
  transition: all 0.15s;
}

.stat-card:hover {
  border-color: var(--accent);
  transform: translateY(-2px);
}

.stat-icon {
  font-size: 28px;
  margin-bottom: 8px;
}

.stat-value {
  font-size: 28px;
  font-weight: 700;
  color: var(--accent);
}

.stat-label {
  font-size: 12px;
  color: var(--text-secondary);
  margin-top: 4px;
  text-transform: uppercase;
  letter-spacing: 0.5px;
}

.quick-actions {
  display: flex;
  flex-wrap: wrap;
  gap: 8px;
}

.success-bar-container {
  background: var(--bg-input);
  border-radius: 4px;
  height: 8px;
  overflow: hidden;
}

.success-bar {
  background: var(--success);
  height: 100%;
  transition: width 0.3s;
}
</style>
