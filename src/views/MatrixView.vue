<script setup lang="ts">
import { ref, computed, onMounted, watch } from 'vue'
import { invoke } from '@tauri-apps/api/core'
import { useExplorerStore } from '../stores/explorer'
import type { PermMatrix } from '../types'
import type { TreeNode } from '../stores/explorer'
import MatrixTree from '../components/MatrixTree.vue'

const store = useExplorerStore()

const matrix = ref<PermMatrix | null>(null)
const loading = ref(false)
const error = ref('')
const selectedPaths = ref<Set<string>>(new Set())
const selectedPrincipals = ref<Set<string>>(new Set())
const pathSearch = ref('')
const principalSearch = ref('')
const showConfig = ref(true)
const filterColor = ref<string | null>(null)
const rowPage = ref(0)
const colPage = ref(0)
const pageSizeRows = 80
const pageSizeCols = 80
const maxCells = 12000

onMounted(() => {
  store.loadShares()
  store.loadUsersGroups()
})

const filteredTreeNodes = computed<TreeNode[]>(() => {
  const q = pathSearch.value.toLowerCase()
  if (!q) return store.tree

  function nodeMatches(node: TreeNode): boolean {
    if (node.name.toLowerCase().includes(q) || node.path.toLowerCase().includes(q)) return true
    return node.children.some(nodeMatches)
  }
  function filterNodes(nodes: TreeNode[]): TreeNode[] {
    return nodes.filter(nodeMatches).map((n) => ({
      ...n,
      children: filterNodes(n.children),
      expanded: q ? true : n.expanded,
    }))
  }
  return filterNodes(store.tree)
})

function toggleExpand(node: TreeNode) {
  store.expandNode(node)
}

const allPrincipals = computed(() => {
  const list: { type: string; name: string; key: string }[] = []
  for (const u of store.users) list.push({ type: 'user', name: u, key: `user:${u}` })
  for (const g of store.groups) list.push({ type: 'group', name: g, key: `group:${g}` })
  return list
})

const filteredPrincipals = computed(() => {
  const q = principalSearch.value.toLowerCase()
  if (!q) return allPrincipals.value
  return allPrincipals.value.filter((p) => p.name.toLowerCase().includes(q))
})

function togglePath(path: string) {
  if (selectedPaths.value.has(path)) selectedPaths.value.delete(path)
  else selectedPaths.value.add(path)
  selectedPaths.value = new Set(selectedPaths.value)
}

function togglePrincipal(key: string) {
  if (selectedPrincipals.value.has(key)) selectedPrincipals.value.delete(key)
  else selectedPrincipals.value.add(key)
  selectedPrincipals.value = new Set(selectedPrincipals.value)
}

function selectAllPaths() {
  const all = new Set<string>()
  function walk(nodes: TreeNode[]) {
    for (const n of nodes) {
      all.add(n.path)
      if (n.expanded) walk(n.children)
    }
  }
  walk(filteredTreeNodes.value)
  selectedPaths.value = all
}
function selectAllPrincipals() { selectedPrincipals.value = new Set(filteredPrincipals.value.map((p) => p.key)) }
function clearAll() {
  selectedPaths.value = new Set()
  selectedPrincipals.value = new Set()
  matrix.value = null
  showConfig.value = true
}

async function generateMatrix() {
  if (selectedPaths.value.size === 0 || selectedPrincipals.value.size === 0) return
  const cells = selectedPaths.value.size * selectedPrincipals.value.size
  if (cells > maxCells) {
    error.value = `La matriz tendría ${cells.toLocaleString()} celdas. Reduce carpetas/usuarios o genera por bloques (máximo ${maxCells.toLocaleString()} celdas).`
    return
  }
  loading.value = true
  error.value = ''
  matrix.value = null
  try {
    const paths = Array.from(selectedPaths.value)
    const principals = Array.from(selectedPrincipals.value).map((key) => {
      const [type, ...nameParts] = key.split(':')
      return [type, nameParts.join(':')] as [string, string]
    })
    matrix.value = await invoke<PermMatrix>('get_perm_matrix', { paths, principals })
    rowPage.value = 0
    colPage.value = 0
    showConfig.value = false
  } catch (e: any) {
    error.value = String(e)
  } finally {
    loading.value = false
  }
}

function exportCSV() {
  if (!matrix.value) return
  const esc = (v: string) => {
    const safe = /^[\s]*[=+@-]/.test(v) || /^[\t\r\n]/.test(v) ? `'${v}` : v
    return `"${safe.replace(/"/g, '""')}"`
  }
  const accessLabel = (color: string) =>
    color === 'green' ? 'RW' : color === 'orange' ? 'R' : 'sin acceso'
  const headers = ['Principal', 'Tipo', 'Carpeta', 'Acceso', 'Permisos']
  const rows: string[][] = []
  for (const row of matrix.value.rows) {
    for (const cell of row.cells) {
      rows.push([
        row.principal,
        row.principal_type,
        cell.path,
        accessLabel(cell.color),
        cell.permissions || '',
      ])
    }
  }
  const csv = [headers, ...rows].map((r) => r.map(esc).join(',')).join('\n')
  const blob = new Blob(['\uFEFF' + csv], { type: 'text/csv;charset=utf-8' })
  const url = URL.createObjectURL(blob)
  const a = document.createElement('a')
  a.href = url
  a.download = `perm-matrix-${new Date().toISOString().slice(0, 10)}.csv`
  a.click()
  URL.revokeObjectURL(url)
}

function shortPath(path: string): string {
  const parts = path.split('/')
  return parts[parts.length - 1]
}

function cellLabel(color: string): string {
  if (color === 'green') return 'RW'
  if (color === 'orange') return 'R'
  return '—'
}

const stats = computed(() => {
  if (!matrix.value) return { green: 0, orange: 0, red: 0, total: 0 }
  let g = 0, o = 0, r = 0
  for (const row of matrix.value.rows) {
    for (const cell of row.cells) {
      if (cell.color === 'green') g++
      else if (cell.color === 'orange') o++
      else r++
    }
  }
  return { green: g, orange: o, red: r, total: g + o + r }
})

function toggleColorFilter(color: string) {
  filterColor.value = filterColor.value === color ? null : color
}

function rowHasFilteredColor(row: PermMatrix['rows'][0]): boolean {
  if (!filterColor.value) return true
  return row.cells.some((c) => c.color === filterColor.value)
}

const filteredRows = computed(() => {
  if (!matrix.value) return []
  return matrix.value.rows.filter(rowHasFilteredColor)
})

watch(filterColor, () => { rowPage.value = 0 })

const visibleRows = computed(() => filteredRows.value.slice(rowPage.value * pageSizeRows, (rowPage.value + 1) * pageSizeRows))
const visiblePaths = computed(() => matrix.value?.paths.slice(colPage.value * pageSizeCols, (colPage.value + 1) * pageSizeCols) ?? [])
const rowPages = computed(() => Math.max(1, Math.ceil(filteredRows.value.length / pageSizeRows)))
const colPages = computed(() => Math.max(1, Math.ceil((matrix.value?.paths.length ?? 0) / pageSizeCols)))
function visibleCells(row: PermMatrix['rows'][0]) {
  return row.cells.slice(colPage.value * pageSizeCols, (colPage.value + 1) * pageSizeCols)
}
</script>

<template>
  <div class="view-header">
    <h1>Matriz de permisos</h1>
    <p>Visualiza quien tiene acceso a que carpetas</p>
  </div>

  <div v-if="error" class="alert alert-error">{{ error }}</div>

  <!-- Barra superior compacta -->
  <div class="card matrix-toolbar">
    <div style="display: flex; align-items: center; gap: 8px; flex-wrap: wrap;">
      <button class="btn btn-sm" :class="showConfig ? 'btn-secondary' : 'btn-primary'" @click="showConfig = !showConfig">
        {{ showConfig ? '▲ Ocultar config' : '▼ Configurar' }}
      </button>

      <template v-if="matrix">
        <div class="matrix-stats">
          <button class="stat-pill" :class="{ active: filterColor === 'green' }" @click="toggleColorFilter('green')">
            <span class="dot green"></span>{{ stats.green }} RW
          </button>
          <button class="stat-pill" :class="{ active: filterColor === 'orange' }" @click="toggleColorFilter('orange')">
            <span class="dot orange"></span>{{ stats.orange }} R
          </button>
          <button class="stat-pill" :class="{ active: filterColor === 'red' }" @click="toggleColorFilter('red')">
            <span class="dot red"></span>{{ stats.red }} sin acceso
          </button>
        </div>
        <div style="flex: 1;"></div>
        <button class="btn btn-secondary btn-sm" @click="exportCSV">CSV</button>
      </template>
    </div>
  </div>

  <!-- Panel de configuracion colapsable -->
  <transition name="slide">
    <div v-if="showConfig" class="card">
      <div class="matrix-config">
        <div class="config-column">
          <div class="config-header">
            <span class="compact-label">Carpetas ({{ selectedPaths.size }})</span>
            <button class="btn btn-secondary btn-sm" @click="selectAllPaths">Todas</button>
          </div>
          <input v-model="pathSearch" placeholder="Filtrar..." class="input-mini" />
          <div class="check-list">
            <MatrixTree
              :nodes="filteredTreeNodes"
              :selected-paths="selectedPaths"
              :depth="0"
              @toggle-path="togglePath"
              @toggle-expand="toggleExpand"
            />
          </div>
        </div>

        <div class="config-column">
          <div class="config-header">
            <span class="compact-label">Usuarios/Grupos ({{ selectedPrincipals.size }})</span>
            <button class="btn btn-secondary btn-sm" @click="selectAllPrincipals">Todos</button>
          </div>
          <input v-model="principalSearch" placeholder="Filtrar..." class="input-mini" />
          <div class="check-list">
            <label v-for="pr in filteredPrincipals.slice(0, 100)" :key="pr.key" class="check-item">
              <input type="checkbox" :checked="selectedPrincipals.has(pr.key)" @change="togglePrincipal(pr.key)" />
              <span class="ptype" :class="pr.type === 'group' ? 'group' : 'user'">{{ pr.type === 'group' ? 'G' : 'U' }}</span>
              <span>{{ pr.name }}</span>
            </label>
          </div>
        </div>
      </div>

      <div class="btn-group" style="margin-top: 10px;">
        <button class="btn btn-primary" @click="generateMatrix" :disabled="loading || selectedPaths.size === 0 || selectedPrincipals.size === 0">
          <span v-if="loading" class="loading-spinner"></span>
          Generar
        </button>
        <button class="btn btn-secondary btn-sm" @click="clearAll">Limpiar</button>
      </div>
    </div>
  </transition>

  <!-- Matriz -->
  <div v-if="matrix" class="matrix-container">
    <div class="matrix-pager">
      <button class="btn btn-secondary btn-sm" :disabled="rowPage === 0" @click="rowPage--">Filas anteriores</button>
      <span>Filas {{ rowPage + 1 }} / {{ rowPages }}</span>
      <button class="btn btn-secondary btn-sm" :disabled="rowPage + 1 >= rowPages" @click="rowPage++">Filas siguientes</button>
      <button class="btn btn-secondary btn-sm" :disabled="colPage === 0" @click="colPage--">Columnas anteriores</button>
      <span>Columnas {{ colPage + 1 }} / {{ colPages }}</span>
      <button class="btn btn-secondary btn-sm" :disabled="colPage + 1 >= colPages" @click="colPage++">Columnas siguientes</button>
    </div>
    <div class="matrix-grid" :style="{ gridTemplateColumns: '160px repeat(' + visiblePaths.length + ', 1fr)' }">
      <!-- Header row -->
      <div class="grid-header sticky-corner">Principal</div>
      <div
        v-for="p in visiblePaths"
        :key="p"
        class="grid-header"
        :title="p"
      >{{ shortPath(p) }}</div>

      <!-- Data rows -->
      <template v-for="row in visibleRows" :key="row.principal_type + ':' + row.principal">
        <div class="grid-label">
          <span class="ptype" :class="row.principal_type === 'group' ? 'group' : 'user'">{{ row.principal_type === 'group' ? 'G' : 'U' }}</span>
          <span class="label-name" :title="row.principal">{{ row.principal }}</span>
        </div>
        <div
          v-for="cell in visibleCells(row)"
          :key="cell.path"
          class="grid-cell"
          :class="cell.color"
          :title="`${row.principal} → ${cell.path}\n${cell.permissions || 'sin acceso'}`"
        >{{ cellLabel(cell.color) }}</div>
      </template>
    </div>
  </div>

  <div v-else-if="!loading && !showConfig" class="card">
    <div class="empty-state"><p>Selecciona carpetas y usuarios para generar la matriz</p></div>
  </div>
</template>

<style scoped>
.matrix-hero { display: flex; align-items: flex-end; justify-content: space-between; gap: 24px; }
.matrix-hero-summary { display: grid; grid-template-columns: repeat(3, minmax(90px, 1fr)); gap: 10px; min-width: 360px; }
.matrix-hero-summary div { padding: 12px 14px; border: 1px solid color-mix(in srgb, var(--border) 72%, transparent); border-radius: 16px; background: color-mix(in srgb, var(--bg-secondary) 82%, transparent); }
.matrix-hero-summary strong { display: block; font-size: 22px; letter-spacing: -0.7px; }
.matrix-hero-summary span { color: var(--text-secondary); font-size: 11px; text-transform: uppercase; letter-spacing: .6px; }
.compact-label { display: block; margin-bottom: 2px; font-size: 10px; color: var(--text-muted); text-transform: uppercase; letter-spacing: 1px; }
.matrix-toolbar { padding: 12px !important; margin-bottom: 12px !important; }
.toolbar-main { display: flex; align-items: center; gap: 10px; flex-wrap: wrap; }
.toolbar-spacer { flex: 1; }
.matrix-pager { display: flex; justify-content: space-between; gap: 10px; align-items: center; flex-wrap: wrap; padding: 12px; background: color-mix(in srgb, var(--bg-secondary) 86%, transparent); border-bottom: 1px solid var(--border); font-size: 12px; color: var(--text-secondary); }
.pager-group { display: flex; align-items: center; gap: 8px; }
.pager-group span { min-width: 148px; text-align: center; font-weight: 600; color: var(--text-primary); }

.matrix-config-card { overflow: hidden; }
.config-title-row { display: flex; align-items: flex-start; justify-content: space-between; gap: 18px; margin-bottom: 18px; }
.cell-budget { display: grid; gap: 2px; min-width: 150px; padding: 12px 14px; border-radius: 16px; background: color-mix(in srgb, var(--accent) 12%, var(--bg-input)); border: 1px solid color-mix(in srgb, var(--accent) 28%, var(--border)); text-align: right; }
.cell-budget span { font-size: 20px; font-weight: 700; }
.cell-budget small { color: var(--text-secondary); }
.cell-budget.warning { background: color-mix(in srgb, var(--danger) 16%, var(--bg-input)); border-color: color-mix(in srgb, var(--danger) 40%, var(--border)); }
.matrix-config { display: grid; grid-template-columns: minmax(0, 1.15fr) minmax(300px, .85fr); gap: 16px; }
.config-column { display: flex; flex-direction: column; gap: 10px; min-width: 0; padding: 14px; border: 1px solid color-mix(in srgb, var(--border) 72%, transparent); border-radius: 18px; background: color-mix(in srgb, var(--bg-input) 50%, transparent); }
.config-header { display: flex; justify-content: space-between; align-items: center; gap: 12px; }
.config-header strong { font-size: 14px; }
.input-mini {
  width: 100%;
  padding: 10px 12px;
  border: 1px solid color-mix(in srgb, var(--border) 76%, transparent);
  border-radius: 12px;
  background: var(--bg-input);
  color: var(--text-primary);
  font-size: 13px;
  outline: none;
}
.input-mini::placeholder { color: var(--text-muted); }
.input-mini:focus {
  border-color: color-mix(in srgb, var(--accent) 58%, var(--border));
  box-shadow: 0 0 0 3px color-mix(in srgb, var(--accent) 13%, transparent);
}

.check-list {
  max-height: 280px; overflow-y: auto; border: 1px solid color-mix(in srgb, var(--border) 70%, transparent);
  border-radius: 16px; padding: 8px; background: var(--bg-secondary);
}
.check-item {
  display: flex; align-items: center; gap: 8px; padding: 7px 8px;
  font-size: 12px; cursor: pointer; border-radius: 11px; color: var(--text-secondary);
}
.check-item:hover { background: var(--bg-hover); }
.check-item input { width: 14px; height: 14px; accent-color: var(--accent); }
.path-text { font-family: monospace; font-size: 10px; white-space: nowrap; }
.more-hint { font-size: 10px; color: var(--text-muted); padding: 4px; text-align: center; }
.matrix-actions { display: flex; align-items: center; gap: 10px; margin-top: 16px; flex-wrap: wrap; }

.ptype {
  display: inline-flex; align-items: center; justify-content: center;
  width: 18px; height: 18px; border-radius: 50%; font-size: 9px; font-weight: 800; flex-shrink: 0;
}
.ptype.user { background: rgba(78, 154, 241, 0.25); color: var(--accent); }
.ptype.group { background: rgba(46, 204, 113, 0.25); color: var(--success); }

.matrix-stats { display: flex; gap: 7px; flex-wrap: wrap; }
.stat-pill {
  display: flex; align-items: center; gap: 6px; padding: 7px 10px;
  background: var(--bg-input); border: 1px solid color-mix(in srgb, var(--border) 72%, transparent); border-radius: 999px;
  font-size: 12px; color: var(--text-secondary); cursor: pointer; transition: all 0.15s;
}
.stat-pill:hover { border-color: var(--accent); }
.stat-pill.active { background: color-mix(in srgb, var(--accent) 12%, var(--bg-tertiary)); color: var(--text-primary); border-color: var(--accent); }
.dot { width: 8px; height: 8px; border-radius: 50%; }
.dot.green { background: #2ecc71; }
.dot.orange { background: #f39c12; }
.dot.red { background: #e74c3c; }

.matrix-container {
  background: var(--bg-secondary); border: 1px solid color-mix(in srgb, var(--border) 72%, transparent);
  border-radius: 22px; padding: 0; overflow: auto;
  max-height: calc(100vh - 250px); min-height: 200px;
  box-shadow: var(--shadow-soft);
}

.matrix-grid {
  display: grid; font-size: 10px; min-width: 100%; width: max-content; background: var(--border);
  gap: 1px;
}

.grid-header {
  position: sticky; top: 0; background: color-mix(in srgb, var(--bg-secondary) 94%, var(--bg-primary));
  color: var(--text-secondary); font-weight: 700; font-size: 10px;
  padding: 11px 8px; text-align: center; white-space: nowrap;
  z-index: 2; text-transform: uppercase; min-width: 58px; letter-spacing: .5px;
}

.sticky-corner {
  position: sticky; left: 0; z-index: 3; min-width: 160px; text-align: left;
}

.grid-label {
  position: sticky; left: 0; background: color-mix(in srgb, var(--bg-secondary) 96%, var(--bg-primary)); z-index: 1;
  padding: 8px 10px; display: flex; align-items: center; gap: 7px;
  min-width: 160px; max-width: 200px;
}
.label-name { font-size: 12px; white-space: nowrap; overflow: hidden; text-overflow: ellipsis; }

.grid-cell {
  display: flex; align-items: center; justify-content: center;
  padding: 8px 4px;
  font-size: 10px; font-weight: 800; font-family: 'Cascadia Code', Consolas, monospace; min-width: 42px;
  transition: transform 0.1s, box-shadow 0.1s; cursor: default; background: var(--bg-primary);
}
.grid-cell:hover { transform: scale(1.16); z-index: 5; position: relative; box-shadow: 0 10px 24px rgba(0, 0, 0, .24); border-radius: 8px; }

.grid-cell.green {
  background: linear-gradient(135deg, rgba(46, 204, 113, 0.28), rgba(46, 204, 113, 0.12)); color: var(--success);
}
.grid-cell.orange {
  background: linear-gradient(135deg, rgba(243, 156, 18, 0.28), rgba(243, 156, 18, 0.11)); color: var(--warning);
}
.grid-cell.red {
  background: color-mix(in srgb, var(--danger) 7%, var(--bg-primary)); color: var(--text-muted);
}
@media (max-width: 900px) {
  .matrix-hero { align-items: stretch; flex-direction: column; }
  .matrix-hero-summary { min-width: 0; grid-template-columns: 1fr; }
  .matrix-config { grid-template-columns: 1fr; }
  .config-title-row { flex-direction: column; }
  .cell-budget { width: 100%; text-align: left; }
}
</style>
