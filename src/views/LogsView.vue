<script setup lang="ts">
import { computed, onMounted, onBeforeUnmount, reactive, ref, shallowRef, watch } from 'vue'
import { onBeforeRouteLeave, useRouter } from 'vue-router'
import { invoke } from '@tauri-apps/api/core'
import { useConnectionStore } from '../stores/connection'
import { useExplorerStore } from '../stores/explorer'
import { resetReadCache } from '../utils/readCache'
import type { AuditLog, AclEntry, ApplyResult } from '../types'
interface HistoryPage { rows: AuditLog[]; total: number; connections: string[]; actions: string[] }
interface Snapshot { snapshots: { path: string; entries: AclEntry[] }[] }
const connection = useConnectionStore()
const explorer = useExplorerStore()
const router = useRouter()
const filter = reactive({ search: '', connection: '', action: '', status: '', from: '', to: '', page: 0, page_size: 50 })
const data = shallowRef<HistoryPage>({ rows: [], total: 0, connections: [], actions: [] })
const detail = shallowRef<AuditLog | null>(null)
const snapshot = shallowRef<Snapshot | null>(null)
const snapshotId = ref<number | null>(null)
const snapshotPage = ref(0)
const loading = ref(false)
const detailLoading = ref(false)
const exporting = ref(false)
const restoring = ref(false)
const confirmRestore = ref(false)
const error = ref('')
const message = ref('')
let generation = 0
let detailGeneration = 0
let disposed = false
let timer: ReturnType<typeof setTimeout> | undefined
onBeforeUnmount(() => { disposed = true; generation++; detailGeneration++; clearTimeout(timer) })
onBeforeRouteLeave(() => {
  if (restoring.value) { error.value = 'Espera a que termine la restauración antes de salir.'; return false }
  return true
})
const totalPages = computed(() => Math.max(1, Math.ceil(data.value.total / filter.page_size)))
const snapshotIds = computed(() => detail.value?.snapshot_ids.split(',').filter(Boolean).map(Number) ?? [])
const visibleSnapshots = computed(() => snapshot.value?.snapshots.slice(snapshotPage.value * 10, (snapshotPage.value + 1) * 10) ?? [])
const canRestore = computed(() => !!detail.value && detail.value.action === 'apply_acl' && detail.value.connection_name === connection.connectedName && connection.isConnected)
const prettyDetail = computed(() => { try { return JSON.stringify(JSON.parse(detail.value?.details || ''), null, 2) } catch { return detail.value?.details ?? '' } })
const retryComparison = computed(() => {
  if (detail.value?.action !== 'user_sync_backup') return null
  try { const parsed = JSON.parse(detail.value.details); return { source: parsed[0].source, target: parsed[0].target } as { source: string; target: string } } catch { return null }
})
async function load() {
  const current = ++generation
  loading.value = true; error.value = ''
  try { const result = await invoke<HistoryPage>('query_history', { filter: { ...filter } }); if (!disposed && current === generation) data.value = result }
  catch (e) { if (current === generation) error.value = String(e) }
  finally { if (current === generation) loading.value = false }
}
function schedule() { clearTimeout(timer); generation++; timer = setTimeout(load, 200) }
watch(() => [filter.search, filter.connection, filter.action, filter.status, filter.from, filter.to], () => { filter.page = 0; schedule() })
watch(() => filter.page, schedule)
watch(() => connection.sessionVersion, () => { snapshot.value = null; confirmRestore.value = false })
onMounted(load)
async function showDetail(id: number) {
  const current = ++detailGeneration
  detailLoading.value = true; detail.value = null; snapshot.value = null; confirmRestore.value = false; snapshotId.value = null
  try { const result = await invoke<AuditLog>('get_history_detail', { id }); if (current === detailGeneration) detail.value = result }
  catch (e) { if (current === detailGeneration) error.value = String(e) }
  finally { if (current === detailGeneration) detailLoading.value = false }
}
async function inspectSnapshot(id: number) {
  const current = detailGeneration
  snapshot.value = null; snapshotId.value = id; confirmRestore.value = false; snapshotPage.value = 0
  try { const result = await invoke<Snapshot>('preview_snapshot', { id }); if (current === detailGeneration && snapshotId.value === id) snapshot.value = result }
  catch (e) { error.value = String(e) }
}
async function restore() {
  if (!snapshotId.value || !confirmRestore.value || !canRestore.value || restoring.value) return
  restoring.value = true; error.value = ''; message.value = ''
  try {
    const result = await invoke<ApplyResult>('restore_snapshot', { snapshotId: snapshotId.value })
    message.value = `${result.paths_modified} carpetas restauradas.`
    if (!result.success) error.value = result.errors.join(' · ')
    resetReadCache(); explorer.clearPermColors(); explorer.currentAcl = []; explorer.diffs = []
    // Refresh without hiding the operation result.
    const resultError = error.value
    await load(); if (resultError) error.value = resultError
  } catch (e) { error.value = String(e) }
  finally { restoring.value = false; confirmRestore.value = false; snapshot.value = null }
}
function download(content: string, name: string, type: string) {
  const url = URL.createObjectURL(new Blob([content], { type }))
  const a = document.createElement('a'); a.href = url; a.download = name; a.click(); URL.revokeObjectURL(url)
}
function cell(value: unknown) {
  let text = String(value ?? '')
  if (/^\s*[=+@-]/.test(text) || /^[\t\r\n]/.test(text)) text = `'${text}`
  return `"${text.replace(/"/g, '""')}"`
}
async function exportFiltered(format: 'json' | 'csv') {
  exporting.value = true; error.value = ''
  try {
    const result = await invoke<{ rows: AuditLog[]; total: number; truncated: boolean }>('export_history', { filter: { ...filter } })
    if (result.truncated) { error.value = 'Más de 10.000 registros. Reduce el intervalo o los filtros para exportar el resultado completo.'; return }
    const stamp = new Date().toISOString().slice(0, 10)
    if (format === 'json') download(JSON.stringify({ exported_at: new Date().toISOString(), filters: filter, ...result }, null, 2), `syno-history-${stamp}.json`, 'application/json')
    else {
      const headers = ['ID', 'Fecha UTC', 'Conexión', 'Acción', 'Ruta', 'Estado', 'Detalles', 'Snapshots']
      const rows = result.rows.map(r => [r.id, r.timestamp, r.connection_name, r.action, r.path, r.success ? 'OK' : 'Error', r.details, r.snapshot_ids])
      download('\uFEFF' + [headers, ...rows].map(r => r.map(cell).join(',')).join('\r\n'), `syno-history-${stamp}.csv`, 'text/csv;charset=utf-8')
    }
    message.value = `${result.rows.length} registros exportados.`
  } catch (e) { error.value = String(e) }
  finally { exporting.value = false }
}
function exportDetail() { if (detail.value) download(detail.value.action === 'user_sync_backup' ? detail.value.details : JSON.stringify(detail.value, null, 2), `syno-operation-${detail.value.id}.json`, 'application/json') }
function closeDetail() { detailGeneration++; detail.value = null; detailLoading.value = false; snapshot.value = null }
</script>
<template>
  <div class="view-header"><div class="eyebrow">TRAZABILIDAD</div><h1>Historial de cambios</h1><p>Busca operaciones, revisa sus detalles, exporta informes y previsualiza snapshots antes de restaurarlos.</p></div>
  <div v-if="error" class="alert alert-error" role="alert">{{ error }}</div><div v-if="message" class="alert alert-success" role="status">{{ message }}</div>
  <section class="card">
    <div class="history-filters"><div class="form-group"><label for="history-search">Buscar ruta, usuario o detalle</label><input id="history-search" v-model="filter.search" placeholder="Buscar…" /></div>
    <div class="form-group"><label for="history-connection">Conexión</label><select id="history-connection" v-model="filter.connection"><option value="">Todas</option><option v-for="c in data.connections" :key="c">{{ c }}</option></select></div>
    <div class="form-group"><label for="history-action">Acción</label><select id="history-action" v-model="filter.action"><option value="">Todas</option><option v-for="a in data.actions" :key="a">{{ a }}</option></select></div>
    <div class="form-group"><label for="history-status">Resultado</label><select id="history-status" v-model="filter.status"><option value="">Todos</option><option value="success">Correcto</option><option value="error">Error / parcial</option></select></div>
    <div class="form-group"><label for="history-from">Desde (UTC)</label><input id="history-from" v-model="filter.from" type="date" /></div><div class="form-group"><label for="history-to">Hasta (UTC)</label><input id="history-to" v-model="filter.to" type="date" /></div></div>
    <div class="btn-group"><button class="btn btn-secondary" :disabled="loading || restoring" @click="load">Actualizar</button><button class="btn btn-secondary" :disabled="exporting" @click="exportFiltered('csv')">Exportar CSV filtrado</button><button class="btn btn-secondary" :disabled="exporting" @click="exportFiltered('json')">Exportar JSON filtrado</button></div>
  </section>
  <section class="card table-scroll" :aria-busy="loading">
    <p class="helper-text"><span v-if="loading" class="loading-spinner" /> {{ data.total }} registros · página {{ filter.page + 1 }} / {{ totalPages }}</p>
    <table><thead><tr><th>Fecha UTC</th><th>NAS</th><th>Acción</th><th>Ruta / usuario</th><th>Resultado</th><th>Detalle</th></tr></thead><tbody><tr v-for="log in data.rows" :key="log.id"><td>{{ log.timestamp }}</td><td>{{ log.connection_name }}</td><td><span class="badge badge-info">{{ log.action }}</span></td><td class="path-cell">{{ log.path }}</td><td><span class="badge" :class="log.success ? 'badge-success' : 'badge-danger'">{{ log.success ? 'OK' : 'Error' }}</span></td><td><button class="btn btn-secondary btn-sm" :disabled="restoring" @click="showDetail(log.id)">Ver #{{ log.id }}</button></td></tr></tbody></table>
    <p v-if="!loading && !data.rows.length" class="empty-state">No hay operaciones con estos filtros.</p>
    <div class="history-pagination"><button class="btn btn-secondary" :disabled="filter.page === 0 || loading" @click="filter.page--">Anterior</button><button class="btn btn-secondary" :disabled="filter.page + 1 >= totalPages || loading" @click="filter.page++">Siguiente</button></div>
  </section>
  <section v-if="detail || detailLoading" class="card detail-panel">
    <button class="btn btn-secondary btn-sm" :disabled="restoring" @click="closeDetail">Cerrar detalle</button>
    <p v-if="detailLoading" role="status">Cargando detalle…</p>
    <template v-if="detail"><h2 class="card-title">Operación #{{ detail.id }} · {{ detail.action }}</h2><p class="helper-text">{{ detail.connection_name }} · {{ detail.timestamp }} UTC · {{ detail.path }}</p>
    <div class="btn-group"><button class="btn btn-secondary" @click="exportDetail">Exportar este registro / respaldo</button><button v-if="retryComparison && detail.connection_name === connection.connectedName" class="btn btn-primary" @click="router.push({ name: 'compare', query: retryComparison })">Volver a comparar estos usuarios</button></div>
    <details><summary>Contenido completo del registro</summary><pre>{{ prettyDetail }}</pre></details>
    <p v-if="detail.action === 'user_sync_backup'" class="helper-text">Respaldo previo a la sincronización. Contiene grupos y ACL originales para recuperación manual. «Volver a comparar» consulta el estado actual y genera un plan nuevo.</p>
    <div v-if="snapshotIds.length"><h3 class="card-title">Snapshots asociados</h3><p v-if="!canRestore" class="helper-text">Para restaurar un snapshot de apply_acl, conecta al NAS {{ detail.connection_name }}.</p><div class="btn-group"><button v-for="id in snapshotIds" :key="id" class="btn btn-secondary" :disabled="!canRestore || restoring" @click="inspectSnapshot(id)">Previsualizar snapshot #{{ id }}</button></div></div>
    <template v-if="snapshot"><p class="helper-text">{{ snapshot.snapshots.length }} carpetas en el snapshot #{{ snapshotId }}.</p><div class="snapshot-preview"><div v-for="s in visibleSnapshots" :key="s.path"><strong>{{ s.path }}</strong><pre>{{ JSON.stringify(s.entries, null, 2) }}</pre></div></div><div class="btn-group"><button class="btn btn-secondary btn-sm" :disabled="snapshotPage === 0" @click="snapshotPage--">Anterior</button><button class="btn btn-secondary btn-sm" :disabled="(snapshotPage + 1) * 10 >= snapshot.snapshots.length" @click="snapshotPage++">Siguiente</button></div>
    <p class="alert alert-warning">La restauración del editor ACL sustituye las ACL actuales de estas carpetas por el snapshot. Revisa el contenido antes de continuar.</p><label class="form-check"><input v-model="confirmRestore" type="checkbox" :disabled="restoring" /> Confirmo la restauración del snapshot #{{ snapshotId }}.</label><button class="btn btn-danger" :disabled="!confirmRestore || restoring || !canRestore" @click="restore">{{ restoring ? 'Restaurando…' : 'Restaurar snapshot revisado' }}</button></template>
    </template>
  </section>
</template>
<style scoped>
.history-filters { display: grid; grid-template-columns: repeat(3, minmax(0, 1fr)); gap: 0 16px; }
.history-pagination { display: flex; justify-content: flex-end; gap: 8px; margin-top: 18px; }
.path-cell { max-width: 250px; overflow-wrap: anywhere; }
.detail-panel { border-color: var(--accent); }
.detail-panel > * { margin-bottom: 14px; }
pre { white-space: pre-wrap; overflow-wrap: anywhere; max-height: 360px; overflow: auto; font-size: 12px; background: var(--bg-input); padding: 14px; border-radius: 8px; }
summary { cursor: pointer; color: var(--accent); padding: 12px 0; }
.snapshot-preview { max-height: 480px; overflow: auto; }
</style>
