<script setup lang="ts">
import { computed, onMounted, onBeforeUnmount, ref, shallowRef, watch } from 'vue'
import { useRoute, onBeforeRouteLeave } from 'vue-router'
import { invoke } from '@tauri-apps/api/core'
import { useConnectionStore } from '../stores/connection'
import { useExplorerStore } from '../stores/explorer'
import type { ShareFolder } from '../types'
import PermissionSummary, { type PermissionInfo } from '../components/PermissionSummary.vue'
import { cachedRead } from '../utils/readCache'

interface Preview {
  id: number
  source_groups: string[]
  target_groups: string[]
  folders: { path: string; source: PermissionInfo[]; target: PermissionInfo[] }[]
  changes: { scope: string; action: string; before: string; after: string }[]
}
const route = useRoute()
const connection = useConnectionStore()
const explorer = useExplorerStore()
const users = ref<string[]>([])
const shares = ref<ShareFolder[]>([])
const source = ref('')
const target = ref(typeof route.query.target === 'string' ? route.query.target : '')
const paths = ref<string[]>([])
const customPaths = ref('')
const recursive = ref(true)
const groups = ref(true)
const mode = ref('merge')
const busy = ref(false)
const applying = ref(false)
const error = ref('')
const result = ref('')
const preview = shallowRef<Preview | null>(null)
let scanId = ''
let disposed = false
const folderFilter = ref('')
const onlyManual = ref(false)
const groupRows = computed(() => [...new Set([...(preview.value?.source_groups ?? []), ...(preview.value?.target_groups ?? [])])].sort().map(name => ({
  name, source: preview.value!.source_groups.includes(name), target: preview.value!.target_groups.includes(name),
})))
const filteredFolders = computed(() => preview.value?.folders.filter(folder => folder.path.toLowerCase().includes(folderFilter.value.toLowerCase()) && (!onlyManual.value || [...folder.source, ...folder.target].some(e => e.origin === 'manual'))) ?? [])
const confirmed = ref(false)
const filter = ref('')
const page = ref(0)
const folderPage = ref(0)
const modes = [
  { id: 'merge', title: 'Añadir', text: 'Copia grupos y entradas que faltan. Conserva los permisos actuales.' },
  { id: 'update', title: 'Actualizar', text: 'Sustituye entradas con el mismo tipo allow/deny y herencia. Añade grupos; conserva los demás.' },
  { id: 'replace', title: 'Reemplazar', text: 'Iguala grupos y ACL directas al origen. Retira las asignaciones adicionales del destino en el ámbito elegido.' },
]
const scope = computed(() => [...new Set([...paths.value, ...customPaths.value.split('\n').map(p => p.trim()).filter(Boolean)])])
const canCompare = computed(() => connection.isConnected && source.value && target.value && source.value !== target.value && (groups.value || scope.value.length > 0))
const filteredChanges = computed(() => preview.value?.changes.filter(c => `${c.scope} ${c.action}`.toLowerCase().includes(filter.value.toLowerCase())) ?? [])
const visibleChanges = computed(() => filteredChanges.value.slice(page.value * 50, (page.value + 1) * 50))
const visibleFolders = computed(() => filteredFolders.value.slice(folderPage.value * 30, (folderPage.value + 1) * 30))
watch([source, target, paths, customPaths, recursive, groups, mode, () => connection.connectedName, () => connection.isConnected], () => {
  preview.value = null
  confirmed.value = false
  result.value = ''
}, { deep: true })
watch(filter, () => { page.value = 0 })
watch([folderFilter, onlyManual], () => { folderPage.value = 0 })
onBeforeRouteLeave(() => {
  if (applying.value) { error.value = 'Espera a que termine la aplicación de cambios antes de salir.'; return false }
  return true
})
function cancelScan() {
  if (scanId) void invoke('cancel_comparison', { scanId }).catch(() => {})
}
onBeforeUnmount(() => { disposed = true; cancelScan() })

async function load() {
  busy.value = true
  error.value = ''
  try {
    const [u, s] = await Promise.all([cachedRead<string[]>('list_users'), cachedRead<ShareFolder[]>('list_shares')])
    if (disposed) return
    users.value = [...u].sort((a, b) => a.localeCompare(b))
    shares.value = s
  } catch (e) { error.value = String(e) }
  finally { busy.value = false }
}
onMounted(load)

async function compare() {
  if (!canCompare.value || busy.value) return
  busy.value = true
  error.value = ''; result.value = ''; preview.value = null; confirmed.value = false
  page.value = 0; folderPage.value = 0
  scanId = crypto.randomUUID()
  try {
    const data = await invoke<Preview>('compare_users', { request: {
      scan_id: scanId,
      source: source.value, target: target.value, paths: scope.value, recursive: recursive.value, groups: groups.value, mode: mode.value,
    } })
    if (!disposed) preview.value = data
  } catch (e) { error.value = String(e) }
  finally { busy.value = false }
}

async function apply() {
  if (!preview.value || !confirmed.value || busy.value) return
  busy.value = true; applying.value = true; error.value = ''
  try {
    const response = await invoke<{ completed: number; total: number; errors: string[] }>('apply_user_comparison', { id: preview.value.id })
    result.value = `${response.completed} de ${response.total} operaciones completadas en ${target.value}.`
    if (response.errors.length) error.value = `Aplicación parcial: ${response.errors.join(' · ')}. Revisa el estado con una nueva comparación.`
  } catch (e) { error.value = `${String(e)}. Vuelve a comparar antes de continuar.` }
  finally {
    explorer.clearPermColors(); explorer.currentAcl = []; explorer.diffs = []
    preview.value = null; confirmed.value = false; busy.value = false; applying.value = false
  }
}
</script>

<template>
  <div class="view-header">
    <div class="eyebrow">GESTIÓN DE ACCESOS</div>
    <h1>Comparar y copiar permisos</h1>
    <p>Usa un usuario como referencia y revisa exactamente qué cambiará en el destino.</p>
  </div>
  <div v-if="error" class="alert alert-error" role="alert">{{ error }}</div>
  <div v-if="result" class="alert alert-success" role="status">{{ result }}</div>
  <fieldset :disabled="busy" class="compare-fields">
    <section class="card">
      <h2 class="card-title"><span class="step-chip">1</span> Elige los usuarios</h2>
      <div class="identity-grid">
        <div class="form-group"><label for="source-user">Usuario de referencia · origen</label><select id="source-user" v-model="source"><option value="">Selecciona un usuario</option><option v-for="u in users" :key="u" :disabled="u === target">{{ u }}</option></select></div>
        <span class="direction" aria-hidden="true">→</span>
        <div class="form-group"><label for="target-user">Usuario que recibirá los permisos · destino</label><select id="target-user" v-model="target"><option value="">Selecciona un usuario</option><option v-for="u in users" :key="u" :disabled="u === source">{{ u }}</option></select></div>
      </div>
      <button class="btn btn-secondary btn-sm" @click="load">Actualizar usuarios y carpetas</button>
    </section>
    <section class="card">
      <h2 class="card-title"><span class="step-chip">2</span> Define el alcance</h2>
      <div class="scope-options">
        <label class="form-check"><input v-model="groups" type="checkbox" /> Copiar pertenencia a grupos</label>
        <label class="form-check"><input v-model="recursive" type="checkbox" /> Incluir subcarpetas</label>
      </div>
      <p class="helper-text">Los grupos afectan al acceso en todo el NAS. Las ACL directas se copian únicamente en las carpetas seleccionadas.</p>
      <div class="btn-group"><button class="btn btn-secondary btn-sm" @click="paths = shares.map(s => s.path)">Seleccionar todas las carpetas compartidas</button><button class="btn btn-secondary btn-sm" @click="paths = []">Limpiar selección</button></div>
      <div class="share-choices">
        <label v-for="share in shares" :key="share.path" class="share-choice"><input v-model="paths" type="checkbox" :value="share.path" /><span>{{ share.name }}<small>{{ share.path }}</small></span></label>
      </div>
      <div class="form-group"><label for="custom-paths">Carpetas específicas o de otros volúmenes · una ruta absoluta por línea</label><textarea id="custom-paths" v-model="customPaths" rows="2" placeholder="/volume1/Equipo/Proyectos" /></div>
      <div class="mode-grid"><label v-for="item in modes" :key="item.id" class="mode-option" :class="{ chosen: mode === item.id }"><input v-model="mode" type="radio" :value="item.id" name="copy-mode" /><strong>{{ item.title }}</strong><span>{{ item.text }}</span></label></div>
    </section>
    <button class="btn btn-primary" :disabled="!canCompare" @click="compare">Comparar y preparar cambios</button>
  </fieldset>
  <div v-if="busy" class="card busy-banner" role="status" aria-live="polite"><span class="loading-spinner" />{{ applying ? 'Aplicando primero grupos y después permisos manuales…' : 'Leyendo usuarios y permisos del NAS… Puedes cambiar de sección durante la lectura.' }}<button v-if="scanId && !applying" class="btn btn-secondary btn-sm" @click="cancelScan">Cancelar análisis</button></div>
  <template v-if="preview">
    <section class="card preview-section">
      <div class="section-heading"><h2 class="card-title">Vista previa</h2><span class="badge badge-info">{{ preview.changes.length }} cambios · {{ preview.folders.length }} carpetas</span></div>
      <div class="identity-grid"><div><h3>{{ source }}</h3><p class="helper-text">Grupos: {{ preview.source_groups.join(', ') || 'Ninguno' }}</p></div><span class="direction">→</span><div><h3>{{ target }}</h3><p class="helper-text">Grupos actuales: {{ preview.target_groups.join(', ') || 'Ninguno' }}</p></div></div>
      <h3 class="card-title">1. Pertenencia a grupos</h3>
      <div class="table-scroll"><table><thead><tr><th>Grupo</th><th>{{ source }}</th><th>{{ target }}</th><th>Resultado previsto en destino</th></tr></thead><tbody><tr v-for="group in groupRows" :key="group.name"><td>{{ group.name }}</td><td>{{ group.source ? 'Miembro' : 'No pertenece' }}</td><td>{{ group.target ? 'Miembro' : 'No pertenece' }}</td><td>{{ !groups ? 'Sin cambios (copia de grupos desactivada)' : group.source && !group.target ? 'Añadir al grupo' : !group.source && group.target && mode === 'replace' ? 'Retirar del grupo' : 'Conservar' }}</td></tr></tbody></table></div>
      <p class="helper-text">Al aplicar: primero se actualizan los grupos y después las entradas manuales de las carpetas. Los permisos obtenidos por grupo no se duplican como permisos personales.</p>
      <h3 class="card-title">2. Permisos de carpetas específicas</h3>
      <div class="form-group"><label for="folder-filter">Buscar carpeta</label><input id="folder-filter" v-model="folderFilter" placeholder="Nombre o ruta de la carpeta…" /></div>
      <label class="form-check"><input v-model="onlyManual" type="checkbox" /> Solo carpetas con permisos manuales de alguno de los dos usuarios</label>
      <div v-if="!preview.folders.length" class="alert alert-warning">Solo has comparado grupos. Selecciona carpetas —o todas las compartidas— para localizar los permisos manuales, e incluye subcarpetas para analizar excepciones.</div>
      <div class="table-scroll"><table><thead><tr><th>Carpeta</th><th>{{ source }} · referencia</th><th>{{ target }} · estado actual</th></tr></thead><tbody><tr v-for="folder in visibleFolders" :key="folder.path"><td class="folder-path">{{ folder.path }}</td><td><PermissionSummary :entries="folder.source" /></td><td><PermissionSummary :entries="folder.target" /></td></tr></tbody></table></div>
      <div class="pagination"><span>{{ filteredFolders.length }} carpetas</span><button class="btn btn-secondary btn-sm" :disabled="folderPage === 0" @click="folderPage--">Anterior</button><span>{{ folderPage + 1 }} / {{ Math.max(1, Math.ceil(filteredFolders.length / 30)) }}</span><button class="btn btn-secondary btn-sm" :disabled="(folderPage + 1) * 30 >= filteredFolders.length" @click="folderPage++">Siguiente</button></div>
      <h3 class="card-title">3. Cambios que se aplicarán</h3>
      <p class="helper-text">Las entradas heredadas se muestran como contexto: se copian en su carpeta de origen, no se convierten en permisos directos. Incluye los padres para reproducir esa herencia. Esta vista no calcula el acceso efectivo de DSM ni cambia propietarios.</p>
      <div v-if="!preview.changes.length" class="empty-state">No hay cambios que aplicar con este modo y alcance.</div>
      <template v-else>
        <div class="form-group"><label for="change-filter">Buscar cambios</label><input id="change-filter" v-model="filter" placeholder="Carpeta, grupo o acción…" /></div>
        <div class="table-scroll"><table><thead><tr><th>Ámbito</th><th>Acción</th><th>Antes</th><th>Después</th></tr></thead><tbody><tr v-for="(change, i) in visibleChanges" :key="i"><td>{{ change.scope }}</td><td><span class="badge" :class="change.action.startsWith('Retirar') ? 'badge-danger' : 'badge-success'">{{ change.action }}</span></td><td><code>{{ change.before || '—' }}</code></td><td><code>{{ change.after || '—' }}</code></td></tr></tbody></table></div>
        <div class="pagination"><button class="btn btn-secondary btn-sm" :disabled="page === 0" @click="page--">Anterior</button><span>{{ page + 1 }} / {{ Math.max(1, Math.ceil(filteredChanges.length / 50)) }}</span><button class="btn btn-secondary btn-sm" :disabled="(page + 1) * 50 >= filteredChanges.length" @click="page++">Siguiente</button></div>
      </template>
    </section>
    <section v-if="preview.changes.length" class="card apply-panel">
      <p>Se guardará el estado anterior en auditoría. Si una operación falla, se detendrá el proceso; los cambios ya realizados se conservarán. La vista previa caduca en 15 minutos.</p>
      <label class="form-check"><input v-model="confirmed" type="checkbox" :disabled="busy" /> He revisado los cambios para <strong>{{ target }}</strong>.</label>
      <button class="btn" :class="mode === 'replace' ? 'btn-danger' : 'btn-primary'" :disabled="!confirmed || busy" @click="apply">Aplicar {{ preview.changes.length }} cambios</button>
    </section>
  </template>
</template>

<style scoped>
.compare-fields { border: 0; min-width: 0; }
.folder-path { min-width: 140px; max-width: 240px; overflow-wrap: anywhere; }
.identity-grid { display: grid; grid-template-columns: 1fr 32px 1fr; gap: 20px; align-items: center; }
.direction { color: var(--accent); font-size: 26px; text-align: center; }
.scope-options, .section-heading { display: flex; gap: 24px; align-items: center; flex-wrap: wrap; }
.section-heading { justify-content: space-between; }
.share-choices { display: grid; grid-template-columns: repeat(auto-fill, minmax(200px, 1fr)); gap: 10px; max-height: 240px; overflow: auto; margin: 18px 0; }
.share-choice { display: flex; align-items: center; gap: 12px; border: 1px solid var(--border); border-radius: 10px; padding: 12px; overflow-wrap: anywhere; }
.share-choice small { display: block; color: var(--text-muted); margin-top: 3px; }
.mode-grid { display: grid; grid-template-columns: repeat(3, 1fr); gap: 12px; }
.mode-option { padding: 18px; border: 1px solid var(--border); border-radius: 12px; cursor: pointer; }
.mode-option strong { margin-left: 8px; }
.mode-option span { display: block; margin-top: 10px; color: var(--text-secondary); font-size: 12px; line-height: 1.6; }
.mode-option.chosen { border-color: var(--accent); background: var(--bg-tertiary); }
.preview-section, .busy-banner { margin-top: 24px; }
.busy-banner { display: flex; align-items: center; gap: 12px; }
.pagination { display: flex; justify-content: flex-end; align-items: center; gap: 12px; margin: 12px 0; color: var(--text-secondary); }
.acl-line { display: block; margin: 6px 0; overflow-wrap: anywhere; }
.apply-panel { display: grid; gap: 16px; border-color: var(--accent); }
.apply-panel p { color: var(--text-secondary); }
.apply-panel .btn { justify-self: start; }
summary { cursor: pointer; padding: 12px 0; color: var(--accent); }
@media (max-width: 1000px) { .mode-grid { grid-template-columns: 1fr; } .identity-grid { gap: 10px; } }
</style>
