<script setup lang="ts">
import { ref, computed, onMounted } from 'vue'
import { useRouter } from 'vue-router'
import { useExplorerStore } from '../stores/explorer'
import PermissionEditor from '../components/PermissionEditor.vue'
import type { AclEntry, ApplyResult } from '../types'

const router = useRouter()
const store = useExplorerStore()

interface SubjectRef {
  type: 'user' | 'group'
  name: string
}
interface AclConfig {
  subjects: SubjectRef[]
  inheritFrom: string
  type: 'allow' | 'deny'
  applyTo: string
  permissions: Record<string, boolean>
  flags: string[]
}

const PERM_ORDER = ['r', 'w', 'x', 'p', 'd', 'D', 'a', 'A', 'R', 'W', 'c', 'C', 'o', 's']
const FLAG_ORDER = ['f', 'd', 'i', 'n']

const entries = ref<AclEntry[]>([])
const lastConfig = ref<AclConfig | null>(null)
const recursive = ref(false)
const showPreview = ref(false)
const showResult = ref(false)
const applyResult = ref<ApplyResult | null>(null)

const selectedPathsList = computed(() => Array.from(store.selectedPaths))

onMounted(() => {
  if (store.selectedPaths.size === 0) {
    router.push('/explorer')
    return
  }
  store.loadUsersGroups()
})

function configToEntries(config: AclConfig): AclEntry[] {
  const permString = PERM_ORDER.map((c) =>
    config.permissions[c] ? c : '-'
  ).join('')

  const flagString = FLAG_ORDER.map((c) =>
    config.flags.includes(c) ? c : '-'
  ).join('')

  return config.subjects.map((s) => ({
    principal_type: `${s.type}:${config.type}`,
    name: s.name,
    permissions: permString,
    flags: flagString,
  }))
}

function onEditorDone(config: AclConfig) {
  lastConfig.value = config
  entries.value = configToEntries(config)
  showResult.value = false
  showPreview.value = false
}

function onEditorCancel() {
  router.push('/explorer')
}

async function preview() {
  showResult.value = false
  showPreview.value = true
  await store.previewChanges(selectedPathsList.value, entries.value, recursive.value)
}

async function apply() {
  const pathCount = selectedPathsList.value.length
  const recStr = recursive.value ? ' (recursivo)' : ''
  if (
    !confirm(
      `Aplicar cambios a ${pathCount} carpeta(s)${recStr}?\n` +
        `${entries.value.length} entrada(s) ACL.\n` +
        `Esto modificara los permisos reales.`
    )
  ) {
    return
  }
  showPreview.value = false
  applyResult.value = await store.applyChanges(
    selectedPathsList.value,
    entries.value,
    recursive.value
  )
  showResult.value = true
}

function backToExplorer() {
  router.push('/explorer')
}
</script>

<template>
  <div class="view-header">
    <h1>Editor de permisos</h1>
    <p>Configura los permisos y aplicalos a las carpetas seleccionadas</p>
  </div>

  <div v-if="store.error" class="alert alert-error">{{ store.error }}</div>

  <!-- Resumen de carpetas seleccionadas -->
  <div class="card">
    <div style="display: flex; justify-content: space-between; align-items: center;">
      <div class="card-title" style="margin: 0">
        Carpetas seleccionadas ({{ selectedPathsList.length }})
      </div>
      <div class="btn-group">
        <label class="form-check" style="font-size: 12px;">
          <input type="checkbox" v-model="recursive" />
          <span>Recursivo</span>
        </label>
        <button class="btn btn-secondary btn-sm" @click="backToExplorer">
          &larr; Cambiar carpetas
        </button>
      </div>
    </div>
    <div class="scrollable" style="max-height: 120px; margin-top: 10px;">
      <div
        v-for="p in selectedPathsList"
        :key="p"
        style="padding: 3px 8px; font-family: monospace; font-size: 12px;"
      >
        {{ p }}
      </div>
    </div>
  </div>

  <!-- Componente Editor de Permisos -->
  <PermissionEditor
    :users="store.users"
    :groups="store.groups"
    @done="onEditorDone"
    @cancel="onEditorCancel"
  />

  <!-- Resumen de entradas generadas -->
  <div class="card" v-if="entries.length > 0">
    <div class="card-title">Entradas generadas ({{ entries.length }})</div>
    <table>
      <thead>
        <tr>
          <th>Tipo</th>
          <th>Nombre</th>
          <th>Permisos</th>
          <th>Flags</th>
        </tr>
      </thead>
      <tbody>
        <tr v-for="(e, i) in entries" :key="i">
          <td>{{ e.principal_type }}</td>
          <td>{{ e.name }}</td>
          <td style="font-family: monospace;">{{ e.permissions }}</td>
          <td style="font-family: monospace;">{{ e.flags }}</td>
        </tr>
      </tbody>
    </table>

    <div class="btn-group" style="margin-top: 14px;">
      <button class="btn btn-secondary" @click="preview">
        Preview (dry-run)
      </button>
      <button class="btn btn-primary" @click="apply" :disabled="store.applying">
        <span v-if="store.applying" class="loading-spinner"></span>
        Aplicar cambios
      </button>
    </div>
  </div>

  <!-- Preview / dry-run -->
  <div class="card" v-if="showPreview && store.diffs.length > 0">
    <div class="card-title">
      Preview de cambios ({{ store.diffs.length }} modificaciones)
    </div>
    <div class="scrollable" style="max-height: 400px;">
      <table class="diff-table">
        <thead>
          <tr>
            <th>Accion</th>
            <th>Ruta</th>
            <th>Principal</th>
            <th>Antes</th>
            <th>Despues</th>
          </tr>
        </thead>
        <tbody>
          <tr v-for="(d, i) in store.diffs" :key="i">
            <td>
              <span
                class="badge"
                :class="{
                  'badge-success': d.action === 'add',
                  'badge-danger': d.action === 'remove',
                  'badge-warning': d.action === 'modify',
                }"
              >{{ d.action }}</span>
            </td>
            <td style="font-family: monospace; font-size: 11px;">{{ d.path }}</td>
            <td>{{ d.principal_type }}:{{ d.name }}</td>
            <td style="font-family: monospace;">{{ d.old_permissions || '-' }}</td>
            <td style="font-family: monospace;">{{ d.new_permissions || '-' }}</td>
          </tr>
        </tbody>
      </table>
    </div>
    <div class="btn-group" style="margin-top: 12px;">
      <button class="btn btn-primary" @click="apply" :disabled="store.applying">
        <span v-if="store.applying" class="loading-spinner"></span>
        Confirmar y aplicar
      </button>
      <button class="btn btn-secondary" @click="showPreview = false">Cerrar preview</button>
    </div>
  </div>

  <div class="card" v-if="showPreview && store.diffs.length === 0">
    <div class="alert alert-warning">
      No hay cambios que aplicar. Los permisos ya estan como se solicito.
    </div>
  </div>

  <!-- Resultado de aplicar -->
  <div class="card" v-if="showResult && applyResult">
    <div v-if="applyResult.success" class="alert alert-success">
      Cambios aplicados correctamente. {{ applyResult.paths_modified }} carpeta(s) modificada(s).
    </div>
    <div v-else class="alert alert-error">
      Completado con errores. {{ applyResult.paths_modified }} modificada(s),
      {{ applyResult.errors.length }} error(es).
    </div>
    <div v-if="applyResult.errors.length > 0" style="margin-top: 12px;">
      <div
        v-for="(err, i) in applyResult.errors"
        :key="i"
        style="font-family: monospace; font-size: 12px; color: var(--danger); padding: 4px 0;"
      >
        {{ err }}
      </div>
    </div>
    <div style="margin-top: 12px;" class="btn-group">
      <button class="btn btn-secondary" @click="router.push('/logs')">Ver logs</button>
      <button class="btn btn-secondary" @click="router.push('/explorer')">Volver</button>
    </div>
  </div>
</template>
