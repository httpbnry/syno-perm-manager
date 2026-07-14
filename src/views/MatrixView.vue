<script setup lang="ts">
import { ref, computed, onMounted } from 'vue'
import { invoke } from '@tauri-apps/api/core'
import { useExplorerStore } from '../stores/explorer'
import type { PermMatrix } from '../types'

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

onMounted(() => {
  store.loadShares()
  store.loadUsersGroups()
})

const allPaths = computed(() => {
  const paths: string[] = []
  function walk(nodes: typeof store.tree) {
    for (const node of nodes) {
      paths.push(node.path)
      if (node.expanded) walk(node.children)
    }
  }
  walk(store.tree)
  return paths
})

const filteredPaths = computed(() => {
  const q = pathSearch.value.toLowerCase()
  if (!q) return allPaths.value
  return allPaths.value.filter((p) => p.toLowerCase().includes(q))
})

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

function selectAllPaths() { selectedPaths.value = new Set(filteredPaths.value) }
function selectAllPrincipals() { selectedPrincipals.value = new Set(filteredPrincipals.value.map((p) => p.key)) }
function clearAll() {
  selectedPaths.value = new Set()
  selectedPrincipals.value = new Set()
  matrix.value = null
  showConfig.value = true
}

async function generateMatrix() {
  if (selectedPaths.value.size === 0 || selectedPrincipals.value.size === 0) return
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
    showConfig.value = false
  } catch (e: any) {
    error.value = String(e)
  } finally {
    loading.value = false
  }
}

function exportCSV() {
  if (!matrix.value) return
  const headers = ['Principal', ...matrix.value.paths]
  const rows: string[][] = []
  for (const row of matrix.value.rows) {
    const cells = row.cells.map((c) => c.color)
    rows.push([`${row.principal_type}:${row.principal}`, ...cells])
  }
  const csv = [headers, ...rows].map((r) => r.map((c) => `"${c}"`).join(',')).join('\n')
  const blob = new Blob([csv], { type: 'text/csv' })
  const url = URL.createObjectURL(blob)
  const a = document.createElement('a')
  a.href = url
  a.download = 'perm-matrix.csv'
  a.click()
  URL.revokeObjectURL(url)
}

function shortPath(path: string): string {
  const parts = path.split('/')
  const last = parts[parts.length - 1]
  return last.length > 12 ? last.slice(0, 10) + '..' : last
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
            <label v-for="p in filteredPaths.slice(0, 100)" :key="p" class="check-item">
              <input type="checkbox" :checked="selectedPaths.has(p)" @change="togglePath(p)" />
              <span class="path-text">{{ shortPath(p) }}</span>
            </label>
            <div v-if="filteredPaths.length > 100" class="more-hint">+{{ filteredPaths.length - 100 }} mas</div>
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
    <div class="matrix-grid" :style="{ gridTemplateColumns: '160px repeat(' + matrix.paths.length + ', 1fr)' }">
      <!-- Header row -->
      <div class="grid-header sticky-corner">Principal</div>
      <div
        v-for="p in matrix.paths"
        :key="p"
        class="grid-header"
        :title="p"
      >{{ shortPath(p) }}</div>

      <!-- Data rows -->
      <template v-for="row in filteredRows" :key="row.principal_type + ':' + row.principal">
        <div class="grid-label">
          <span class="ptype" :class="row.principal_type === 'group' ? 'group' : 'user'">{{ row.principal_type === 'group' ? 'G' : 'U' }}</span>
          <span class="label-name" :title="row.principal">{{ row.principal }}</span>
        </div>
        <div
          v-for="cell in row.cells"
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
.compact-label { font-size: 10px; color: var(--text-secondary); text-transform: uppercase; letter-spacing: 0.5px; }
.matrix-toolbar { padding: 8px 12px !important; margin-bottom: 8px !important; }

.matrix-config { display: grid; grid-template-columns: 1fr 1fr; gap: 12px; }
.config-column { display: flex; flex-direction: column; gap: 4px; }
.config-header { display: flex; justify-content: space-between; align-items: center; }

.check-list {
  max-height: 200px; overflow-y: auto; border: 1px solid var(--border);
  border-radius: 6px; padding: 4px;
}
.check-item {
  display: flex; align-items: center; gap: 5px; padding: 2px 6px;
  font-size: 11px; cursor: pointer; border-radius: 3px;
}
.check-item:hover { background: var(--bg-hover); }
.check-item input { width: 13px; height: 13px; accent-color: var(--accent); }
.path-text { font-family: monospace; font-size: 10px; overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
.more-hint { font-size: 10px; color: var(--text-muted); padding: 4px; text-align: center; }

.ptype {
  display: inline-flex; align-items: center; justify-content: center;
  width: 15px; height: 15px; border-radius: 50%; font-size: 8px; font-weight: 700; flex-shrink: 0;
}
.ptype.user { background: rgba(78, 154, 241, 0.25); color: var(--accent); }
.ptype.group { background: rgba(46, 204, 113, 0.25); color: var(--success); }

/* Stats pills */
.matrix-stats { display: flex; gap: 4px; }
.stat-pill {
  display: flex; align-items: center; gap: 4px; padding: 3px 8px;
  background: var(--bg-input); border: 1px solid var(--border); border-radius: 12px;
  font-size: 10px; color: var(--text-secondary); cursor: pointer; transition: all 0.15s;
}
.stat-pill:hover { border-color: var(--accent); }
.stat-pill.active { background: var(--bg-tertiary); color: var(--text-primary); border-color: var(--accent); }
.dot { width: 8px; height: 8px; border-radius: 50%; }
.dot.green { background: #2ecc71; }
.dot.orange { background: #f39c12; }
.dot.red { background: #e74c3c; }

/* Matrix grid */
.matrix-container {
  background: var(--bg-secondary); border: 1px solid var(--border);
  border-radius: var(--radius); padding: 0; overflow: auto;
  max-height: calc(100vh - 250px); min-height: 200px;
}

.matrix-grid {
  display: grid; font-size: 10px; min-width: 100%; width: max-content;
}

.grid-header {
  position: sticky; top: 0; background: var(--bg-secondary);
  color: var(--text-secondary); font-weight: 600; font-size: 9px;
  padding: 6px 4px; text-align: center; border-bottom: 2px solid var(--border);
  border-right: 1px solid var(--border); white-space: nowrap; overflow: hidden;
  text-overflow: ellipsis; z-index: 2; text-transform: uppercase;
}

.sticky-corner {
  position: sticky; left: 0; z-index: 3; min-width: 160px; text-align: left;
}

.grid-label {
  position: sticky; left: 0; background: var(--bg-secondary); z-index: 1;
  padding: 4px 8px; display: flex; align-items: center; gap: 4px;
  border-bottom: 1px solid var(--border); border-right: 1px solid var(--border);
  min-width: 160px; max-width: 160px;
}
.label-name { font-size: 11px; overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }

.grid-cell {
  display: flex; align-items: center; justify-content: center;
  padding: 4px 2px; border-bottom: 1px solid var(--border); border-right: 1px solid var(--border);
  font-size: 9px; font-weight: 600; font-family: monospace; min-width: 36px;
  transition: transform 0.1s; cursor: default;
}
.grid-cell:hover { transform: scale(1.15); z-index: 5; position: relative; }

.grid-cell.green {
  background: rgba(46, 204, 113, 0.2); color: var(--success);
}
.grid-cell.orange {
  background: rgba(243, 156, 18, 0.2); color: var(--warning);
}
.grid-cell.red {
  background: rgba(231, 76, 60, 0.08); color: var(--text-muted);
}
</style>
