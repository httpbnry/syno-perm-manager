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

onMounted(() => {
  store.loadShares()
  store.loadUsersGroups()
})

const allPaths = computed(() => {
  const paths: string[] = []
  function walk(nodes: typeof store.tree) {
    for (const node of nodes) {
      paths.push(node.path)
      if (node.expanded) {
        walk(node.children)
      }
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
  for (const u of store.users) {
    list.push({ type: 'user', name: u, key: `user:${u}` })
  }
  for (const g of store.groups) {
    list.push({ type: 'group', name: g, key: `group:${g}` })
  }
  return list
})

const filteredPrincipals = computed(() => {
  const q = principalSearch.value.toLowerCase()
  if (!q) return allPrincipals.value
  return allPrincipals.value.filter((p) => p.name.toLowerCase().includes(q))
})

function togglePath(path: string) {
  if (selectedPaths.value.has(path)) {
    selectedPaths.value.delete(path)
  } else {
    selectedPaths.value.add(path)
  }
  selectedPaths.value = new Set(selectedPaths.value)
}

function togglePrincipal(key: string) {
  if (selectedPrincipals.value.has(key)) {
    selectedPrincipals.value.delete(key)
  } else {
    selectedPrincipals.value.add(key)
  }
  selectedPrincipals.value = new Set(selectedPrincipals.value)
}

function selectAllPaths() {
  selectedPaths.value = new Set(filteredPaths.value)
}

function selectAllPrincipals() {
  selectedPrincipals.value = new Set(filteredPrincipals.value.map((p) => p.key))
}

function clearAll() {
  selectedPaths.value = new Set()
  selectedPrincipals.value = new Set()
  matrix.value = null
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
  const csv = [headers, ...rows]
    .map((r) => r.map((c) => `"${c}"`).join(','))
    .join('\n')
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
  return parts.slice(-2).join('/')
}

const cellColors: Record<string, string> = {
  red: '#e74c3c',
  orange: '#f39c12',
  green: '#2ecc71',
}
</script>

<template>
  <div class="view-header">
    <h1>Matriz de permisos</h1>
    <p>Visualiza quien tiene acceso a que carpetas en un solo grid</p>
  </div>

  <div v-if="error" class="alert alert-error">{{ error }}</div>

  <!-- Selección -->
  <div class="card">
    <div class="card-title">Configurar matriz</div>

    <div class="matrix-config">
      <!-- Paths -->
      <div class="config-column">
        <div style="display: flex; justify-content: space-between; align-items: center; margin-bottom: 6px;">
          <span class="compact-label">Carpetas ({{ selectedPaths.size }})</span>
          <button class="btn btn-secondary btn-sm" @click="selectAllPaths">Todas</button>
        </div>
        <input v-model="pathSearch" placeholder="Filtrar carpetas..." class="input-mini" style="margin-bottom: 6px;" />
        <div class="scrollable check-list">
          <label v-for="p in filteredPaths" :key="p" class="check-item">
            <input type="checkbox" :checked="selectedPaths.has(p)" @change="togglePath(p)" />
            <span style="font-family: monospace; font-size: 11px;">{{ shortPath(p) }}</span>
          </label>
        </div>
      </div>

      <!-- Principals -->
      <div class="config-column">
        <div style="display: flex; justify-content: space-between; align-items: center; margin-bottom: 6px;">
          <span class="compact-label">Usuarios/Grupos ({{ selectedPrincipals.size }})</span>
          <button class="btn btn-secondary btn-sm" @click="selectAllPrincipals">Todos</button>
        </div>
        <input v-model="principalSearch" placeholder="Filtrar..." class="input-mini" style="margin-bottom: 6px;" />
        <div class="scrollable check-list">
          <label v-for="pr in filteredPrincipals" :key="pr.key" class="check-item">
            <input type="checkbox" :checked="selectedPrincipals.has(pr.key)" @change="togglePrincipal(pr.key)" />
            <span class="principal-type" :class="pr.type === 'group' ? 'group' : 'user'">{{ pr.type === 'group' ? 'G' : 'U' }}</span>
            <span>{{ pr.name }}</span>
          </label>
        </div>
      </div>
    </div>

    <div class="btn-group" style="margin-top: 12px;">
      <button class="btn btn-primary" @click="generateMatrix" :disabled="loading || selectedPaths.size === 0 || selectedPrincipals.size === 0">
        <span v-if="loading" class="loading-spinner"></span>
        Generar matriz
      </button>
      <button v-if="matrix" class="btn btn-secondary" @click="exportCSV">Exportar CSV</button>
      <button class="btn btn-secondary" @click="clearAll">Limpiar</button>
    </div>
  </div>

  <!-- Leyenda -->
  <div v-if="matrix" class="card" style="padding: 10px 14px;">
    <div style="display: flex; gap: 16px; font-size: 11px; color: var(--text-secondary);">
      <span><span class="legend-dot" style="background: #2ecc71;"></span> Lectura y escritura</span>
      <span><span class="legend-dot" style="background: #f39c12;"></span> Solo lectura</span>
      <span><span class="legend-dot" style="background: #e74c3c;"></span> Sin acceso</span>
    </div>
  </div>

  <!-- Matriz -->
  <div v-if="matrix" class="card matrix-card">
    <div class="matrix-scroll">
      <table class="matrix-table">
        <thead>
          <tr>
            <th class="sticky-col">Principal</th>
            <th v-for="p in matrix.paths" :key="p" :title="p">{{ shortPath(p) }}</th>
          </tr>
        </thead>
        <tbody>
          <tr v-for="row in matrix.rows" :key="row.principal_type + ':' + row.principal">
            <td class="sticky-col">
              <span class="principal-type" :class="row.principal_type === 'group' ? 'group' : 'user'">{{ row.principal_type === 'group' ? 'G' : 'U' }}</span>
              {{ row.principal }}
            </td>
            <td
              v-for="cell in row.cells"
              :key="cell.path"
              class="matrix-cell"
              :style="{ background: cellColors[cell.color] || 'transparent' }"
              :title="`${row.principal_type}:${row.principal} -> ${cell.path}\nPermisos: ${cell.permissions || 'sin acceso'}`"
            ></td>
          </tr>
        </tbody>
      </table>
    </div>
  </div>
</template>

<style scoped>
.compact-label {
  font-size: 11px;
  color: var(--text-secondary);
  text-transform: uppercase;
  letter-spacing: 0.5px;
}

.matrix-config {
  display: grid;
  grid-template-columns: 1fr 1fr;
  gap: 16px;
}

.config-column {
  display: flex;
  flex-direction: column;
}

.check-list {
  max-height: 250px;
  border: 1px solid var(--border);
  border-radius: 6px;
  padding: 6px;
}

.check-item {
  display: flex;
  align-items: center;
  gap: 6px;
  padding: 3px 6px;
  font-size: 12px;
  cursor: pointer;
  border-radius: 3px;
}

.check-item:hover {
  background: var(--bg-hover);
}

.check-item input {
  width: 14px;
  height: 14px;
  accent-color: var(--accent);
}

.principal-type {
  display: inline-flex;
  align-items: center;
  justify-content: center;
  width: 16px;
  height: 16px;
  border-radius: 50%;
  font-size: 9px;
  font-weight: 700;
  flex-shrink: 0;
}

.principal-type.user {
  background: rgba(78, 154, 241, 0.25);
  color: var(--accent);
}

.principal-type.group {
  background: rgba(46, 204, 113, 0.25);
  color: var(--success);
}

.matrix-card {
  padding: 8px;
  overflow: hidden;
}

.matrix-scroll {
  overflow: auto;
  max-height: 600px;
}

.matrix-table {
  border-collapse: collapse;
  font-size: 11px;
}

.matrix-table th,
.matrix-table td {
  padding: 4px 8px;
  border: 1px solid var(--border);
  white-space: nowrap;
}

.matrix-table th {
  background: var(--bg-secondary);
  color: var(--text-secondary);
  font-weight: 500;
  font-size: 10px;
  position: sticky;
  top: 0;
  z-index: 1;
}

.sticky-col {
  position: sticky;
  left: 0;
  background: var(--bg-secondary);
  z-index: 2;
  min-width: 140px;
  max-width: 200px;
  overflow: hidden;
  text-overflow: ellipsis;
}

.matrix-cell {
  width: 24px;
  height: 24px;
  min-width: 24px;
  padding: 0 !important;
}

.legend-dot {
  display: inline-block;
  width: 10px;
  height: 10px;
  border-radius: 50%;
  margin-right: 4px;
  vertical-align: middle;
}
</style>
