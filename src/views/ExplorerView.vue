<script setup lang="ts">
import { onMounted, computed, ref } from 'vue'
import { useRouter } from 'vue-router'
import { useExplorerStore, type TreeNode } from '../stores/explorer'
import { useConnectionStore } from '../stores/connection'
import TreeView from '../components/TreeView.vue'
import type { SelectedPrincipal } from '../types'

const router = useRouter()
const store = useExplorerStore()
const connStore = useConnectionStore()

const selectedCount = computed(() => store.selectedPaths.size)
const principalSearch = ref('')

const filteredUsers = computed(() => {
  const q = principalSearch.value.toLowerCase()
  if (!q) return store.users
  return store.users.filter((u) => u.toLowerCase().includes(q))
})

const filteredGroups = computed(() => {
  const q = principalSearch.value.toLowerCase()
  if (!q) return store.groups
  return store.groups.filter((g) => g.toLowerCase().includes(q))
})

const progressPct = computed(() => {
  if (store.analyzeProgress.total === 0) return 0
  return Math.round((store.analyzeProgress.current / store.analyzeProgress.total) * 100)
})

onMounted(() => {
  if (connStore.isConnected) {
    store.loadShares()
    store.loadUsersGroups()
  }
})

function handleToggle(node: TreeNode) {
  store.expandNode(node)
}

function handleSelect(path: string) {
  store.toggleSelection(path)
}

function goToEditor() {
  if (selectedCount.value > 0) {
    router.push('/acl-editor')
  }
}

function selectAll() {
  for (const node of store.tree) {
    store.selectAllChildren(node, true)
  }
}

function clearAll() {
  store.clearSelection()
}

function selectPrincipal(p: SelectedPrincipal) {
  store.selectedPrincipal = p
  principalSearch.value = ''
}

function clearAnalysis() {
  store.clearPermColors()
}
</script>

<template>
  <div class="view-header">
    <h1>Explorador de carpetas</h1>
    <p>Selecciona multiples carpetas para aplicar permisos en lote</p>
  </div>

  <div v-if="store.error" class="alert alert-error">{{ store.error }}</div>

  <!-- Barra de analisis de permisos -->
  <div class="card">
    <div style="display: flex; justify-content: space-between; align-items: center; flex-wrap: wrap; gap: 8px;">
      <div class="card-title" style="margin: 0;">Analisis de permisos</div>
      <div style="display: flex; gap: 6px; align-items: center;">
        <button class="btn btn-secondary btn-sm" @click="store.refreshCache()" :disabled="store.analyzing">
          Refrescar cache
        </button>
        <button v-if="store.selectedPrincipal" class="btn btn-secondary btn-sm" @click="clearAnalysis">
          Qitar analisis
        </button>
      </div>
    </div>

    <div style="margin-top: 10px; display: flex; gap: 8px; align-items: flex-end; flex-wrap: wrap;">
      <div class="form-group" style="flex: 1; min-width: 200px;">
        <label>Buscar usuario o grupo</label>
        <input v-model="principalSearch" placeholder="Escribre para filtrar..." />
      </div>
      <button
        class="btn btn-primary"
        @click="store.analyzePerms()"
        :disabled="!store.selectedPrincipal || store.analyzing"
      >
        <span v-if="store.analyzing" class="loading-spinner"></span>
        Analizar permisos
      </button>
    </div>

    <div v-if="store.selectedPrincipal" style="margin-top: 8px; font-size: 12px; color: var(--text-secondary);">
      Analizando: <span class="badge badge-info">{{ store.selectedPrincipal.type === 'group' ? 'Grupo' : 'Usuario' }}: {{ store.selectedPrincipal.name }}</span>
    </div>

    <div v-if="store.analyzing" style="margin-top: 10px;">
      <div style="background: var(--bg-input); border-radius: 4px; height: 6px; overflow: hidden;">
        <div :style="{ width: progressPct + '%', background: 'var(--accent)', height: '100%', transition: 'width 0.2s' }"></div>
      </div>
      <div style="font-size: 11px; color: var(--text-muted); margin-top: 4px;">
        {{ store.analyzeProgress.current }} / {{ store.analyzeProgress.total }} carpetas
      </div>
    </div>

    <div v-if="store.permColors.size > 0" style="margin-top: 10px; display: flex; gap: 12px; align-items: center; flex-wrap: wrap; font-size: 11px; color: var(--text-secondary);">
      <span><span class="perm-legend red"></span> Sin acceso</span>
      <span><span class="perm-legend orange"></span> Solo lectura</span>
      <span><span class="perm-legend green"></span> Lectura y escritura</span>
      <span style="color: var(--warning);">&#9881; Override</span>
      <label class="form-check" style="margin-left: auto;">
        <input type="checkbox" v-model="store.onlyConflicts" />
        <span>Solo conflictos ({{ store.overrides.size }})</span>
      </label>
    </div>

    <div v-if="!store.selectedPrincipal" style="margin-top: 12px; display: flex; gap: 16px; flex-wrap: wrap;">
      <div style="flex: 1; min-width: 200px;">
        <div class="compact-label" style="margin-bottom: 4px;">Usuarios ({{ filteredUsers.length }})</div>
        <div class="principal-list">
          <div
            v-for="u in filteredUsers.slice(0, 30)"
            :key="u"
            class="principal-item"
            @click="selectPrincipal({ type: 'user', name: u })"
          >
            <span class="principal-type user">U</span>
            {{ u }}
          </div>
          <div v-if="filteredUsers.length > 30" style="font-size: 10px; color: var(--text-muted); padding: 4px;">
            ...y {{ filteredUsers.length - 30 }} mas (usa el buscador)
          </div>
        </div>
      </div>
      <div style="flex: 1; min-width: 200px;">
        <div class="compact-label" style="margin-bottom: 4px;">Grupos ({{ filteredGroups.length }})</div>
        <div class="principal-list">
          <div
            v-for="g in filteredGroups"
            :key="g"
            class="principal-item"
            @click="selectPrincipal({ type: 'group', name: g })"
          >
            <span class="principal-type group">G</span>
            {{ g }}
          </div>
        </div>
      </div>
    </div>
  </div>

  <!-- Barra de acciones del arbol -->
  <div class="card">
    <div style="display: flex; justify-content: space-between; align-items: center;">
      <div class="card-title" style="margin: 0">Carpetas compartidas</div>
      <div class="btn-group">
        <button class="btn btn-secondary btn-sm" @click="selectAll">Seleccionar todo</button>
        <button class="btn btn-secondary btn-sm" @click="clearAll">Limpiar</button>
        <button class="btn btn-secondary btn-sm" @click="store.loadShares()">Actualizar</button>
      </div>
    </div>
  </div>

  <div class="card" v-if="store.loading">
    <div class="empty-state">
      <span class="loading-spinner"></span>
      <p>Cargando carpetas...</p>
    </div>
  </div>

  <div class="card" v-else-if="store.tree.length > 0">
    <div class="scrollable" style="max-height: 500px;">
      <TreeView
        :nodes="store.tree"
        :selected-paths="store.selectedPaths"
        :perm-colors="store.permColors"
        :overrides="store.overrides"
        :only-conflicts="store.onlyConflicts"
        @toggle="handleToggle"
        @select="handleSelect"
      />
    </div>
  </div>

  <div class="empty-state" v-else>
    <p>No se encontraron carpetas compartidas. Verifica la conexion.</p>
  </div>

  <div class="selection-bar" v-if="selectedCount > 0">
    <span class="selection-count">
      {{ selectedCount }} carpeta(s) seleccionada(s)
    </span>
    <button class="btn btn-primary" @click="goToEditor">
      Editar permisos &rarr;
    </button>
  </div>
</template>

<style scoped>
.compact-label {
  font-size: 11px;
  color: var(--text-secondary);
  text-transform: uppercase;
  letter-spacing: 0.5px;
}

.principal-list {
  max-height: 200px;
  overflow-y: auto;
  border: 1px solid var(--border);
  border-radius: 6px;
  padding: 4px;
}

.principal-item {
  display: flex;
  align-items: center;
  gap: 6px;
  padding: 4px 8px;
  border-radius: 4px;
  cursor: pointer;
  font-size: 12px;
  transition: background 0.1s;
}

.principal-item:hover {
  background: var(--bg-hover);
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

.perm-legend {
  display: inline-block;
  width: 10px;
  height: 10px;
  border-radius: 50%;
  margin-right: 4px;
  vertical-align: middle;
}

.perm-legend.red { background: #e74c3c; }
.perm-legend.orange { background: #f39c12; }
.perm-legend.green { background: #2ecc71; }
</style>
