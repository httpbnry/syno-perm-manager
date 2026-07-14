<script setup lang="ts">
import { ref, onMounted } from 'vue'
import { invoke } from '@tauri-apps/api/core'
import type { AuditLog, ApplyResult } from '../types'

const logs = ref<AuditLog[]>([])
const loading = ref(false)
const restoring = ref<number | null>(null)
const restoreResult = ref<ApplyResult | null>(null)
const restoreError = ref<string>('')

onMounted(() => {
  loadLogs()
})

async function loadLogs() {
  loading.value = true
  restoreResult.value = null
  restoreError.value = ''
  try {
    logs.value = await invoke<AuditLog[]>('list_logs', { limit: 200 })
  } catch (e: any) {
    console.error(e)
  } finally {
    loading.value = false
  }
}

function hasSnapshots(log: AuditLog): boolean {
  return !!log.snapshot_ids && log.snapshot_ids.trim() !== ''
}

function snapshotCount(log: AuditLog): number {
  if (!log.snapshot_ids) return 0
  return log.snapshot_ids.split(',').filter((s) => s.trim() !== '').length
}

async function undoLog(log: AuditLog) {
  if (!hasSnapshots(log)) return

  const count = snapshotCount(log)
  if (!confirm(`Deshacer la accion #${log.id}?\nSe restauraran ${count} snapshot(s) de ACL.\nEsto sobreescribira los permisos actuales con los que habia antes del cambio.`)) {
    return
  }

  restoring.value = log.id
  restoreResult.value = null
  restoreError.value = ''

  const snapIds = log.snapshot_ids.split(',').filter((s) => s.trim() !== '')

  let totalModified = 0
  let allErrors: string[] = []
  let allSuccess = true

  for (const snapId of snapIds) {
    const id = parseInt(snapId.trim())
    try {
      const result = await invoke<ApplyResult>('restore_snapshot', { snapshotId: id })
      totalModified += result.paths_modified
      if (!result.success) {
        allSuccess = false
        allErrors.push(...result.errors)
      }
    } catch (e: any) {
      allSuccess = false
      allErrors.push(String(e))
    }
  }

  restoreResult.value = {
    success: allSuccess,
    paths_modified: totalModified,
    errors: allErrors,
  }

  if (!allSuccess) {
    restoreError.value = 'Algunos snapshots no se pudieron restaurar completamente'
  }

  restoring.value = null
  await loadLogs()
}
</script>

<template>
  <div class="view-header">
    <h1>Logs de auditoria</h1>
    <p>Historial de cambios de permisos aplicados. Puedes deshacer acciones.</p>
  </div>

  <div v-if="restoreResult" class="card">
    <div v-if="restoreResult.success" class="alert alert-success">
      Deshacer completado. {{ restoreResult.paths_modified }} carpeta(s) restaurada(s).
    </div>
    <div v-else class="alert alert-error">
      Deshacer completado con errores. {{ restoreResult.paths_modified }} restaurada(s),
      {{ restoreResult.errors.length }} error(es).
    </div>
    <div v-if="restoreResult.errors.length > 0" style="margin-top: 8px;">
      <div
        v-for="(err, i) in restoreResult.errors"
        :key="i"
        style="font-family: monospace; font-size: 11px; color: var(--danger); padding: 2px 0;"
      >{{ err }}</div>
    </div>
  </div>

  <div class="card">
    <div style="display: flex; justify-content: space-between; align-items: center;">
      <div class="card-title" style="margin: 0">{{ logs.length }} registros</div>
      <button class="btn btn-secondary btn-sm" @click="loadLogs">Actualizar</button>
    </div>
  </div>

  <div class="card" v-if="loading">
    <div class="empty-state">
      <span class="loading-spinner"></span>
      <p>Cargando logs...</p>
    </div>
  </div>

  <div class="card" v-else-if="logs.length > 0">
    <table>
      <thead>
        <tr>
          <th>Fecha</th>
          <th>Conexion</th>
          <th>Accion</th>
          <th>Ruta</th>
          <th>Detalles</th>
          <th>Estado</th>
          <th>Deshacer</th>
        </tr>
      </thead>
      <tbody>
        <tr v-for="log in logs" :key="log.id">
          <td style="font-family: monospace; font-size: 11px;">{{ log.timestamp }}</td>
          <td>{{ log.connection_name }}</td>
          <td>
            <span
              class="badge"
              :class="{
                'badge-info': log.action === 'apply_acl',
                'badge-warning': log.action === 'restore_snapshot',
                'badge-success': log.action.startsWith('create_') || log.action === 'add_group_member',
                'badge-danger': log.action.startsWith('delete_') || log.action.startsWith('rename_') || log.action === 'set_user_password' || log.action === 'set_group_members',
              }"
            >{{ log.action }}</span>
          </td>
          <td style="font-family: monospace; font-size: 11px; max-width: 250px; overflow: hidden; text-overflow: ellipsis;">{{ log.path }}</td>
          <td style="font-size: 11px;">{{ log.details }}</td>
          <td>
            <span v-if="log.success" class="badge badge-success">OK</span>
            <span v-else class="badge badge-danger">Error</span>
          </td>
          <td>
            <button
              v-if="hasSnapshots(log) && log.action === 'apply_acl'"
              class="btn btn-danger btn-sm"
              @click="undoLog(log)"
              :disabled="restoring !== null"
            >
              <span v-if="restoring === log.id" class="loading-spinner"></span>
              <span v-if="restoring === log.id"> Restaurando...</span>
              <span v-else>&#8617; Deshacer</span>
            </button>
            <span
              v-else-if="log.action === 'restore_snapshot'"
              style="font-size: 10px; color: var(--text-muted);"
            >(restore)</span>
            <span v-else style="font-size: 10px; color: var(--text-muted);">-</span>
          </td>
        </tr>
      </tbody>
    </table>
  </div>

  <div class="empty-state" v-else>
    <p>No hay logs registrados todavia.</p>
  </div>
</template>
