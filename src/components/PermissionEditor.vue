<script setup lang="ts">
import { ref, reactive, computed } from 'vue'

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

const props = defineProps<{
  users: string[]
  groups: string[]
}>()

const emit = defineEmits<{
  cancel: []
  done: [config: AclConfig]
}>()

// ─── 1. CONTROL DE SUJETOS (Tag Input multiselect) ───────────────────────────
const selectedSubjects = ref<SubjectRef[]>([])
const subjectInput = ref('')
const subjectType = ref<'user' | 'group'>('user')
const showDropdown = ref(false)

const availableOptions = computed(() => {
  return subjectType.value === 'group' ? props.groups : props.users
})

const filteredOptions = computed(() => {
  const q = subjectInput.value.toLowerCase()
  return availableOptions.value.filter((n) => {
    if (!q) return true
    return n.toLowerCase().includes(q)
  })
})

function addSubject(name: string) {
  const clean = name.trim()
  if (!clean) return
  const exists = selectedSubjects.value.some(
    (s) => s.type === subjectType.value && s.name === clean
  )
  if (!exists) {
    selectedSubjects.value.push({ type: subjectType.value, name: clean })
  }
  subjectInput.value = ''
  showDropdown.value = false
}

function removeSubject(idx: number) {
  selectedSubjects.value.splice(idx, 1)
}

function onSubjectInputKeydown(e: KeyboardEvent) {
  if (e.key === 'Enter') {
    e.preventDefault()
    if (filteredOptions.value.length === 1) {
      addSubject(filteredOptions.value[0])
    } else if (subjectInput.value.trim()) {
      addSubject(subjectInput.value)
    }
  }
}

function onInputBlur() {
  setTimeout(() => {
    showDropdown.value = false
  }, 200)
}

// ─── 2. LOGICA DE HERENCIA ───────────────────────────────────────────────────
const inheritFrom = ref('Ninguno')
const inheritOptions = [
  'Ninguno',
  '/volume1/share',
  '/volume1',
  '(predeterminado del sistema)',
]
const permsDisabled = computed(() => inheritFrom.value !== 'Ninguno')

// ─── 3. CONFIGURACION DE REGLAS ──────────────────────────────────────────────
const ruleType = ref<'allow' | 'deny'>('allow')
const applyTo = ref('all')
const applyToOptions = [
  { value: 'all', label: 'Esta carpeta, subcarpetas y archivos', flags: ['f', 'd'] },
  { value: 'this', label: 'Solo esta carpeta', flags: [] },
  { value: 'subdirs', label: 'Solo subcarpetas', flags: ['d', 'n'] },
  { value: 'files', label: 'Solo archivos', flags: ['f', 'n'] },
  { value: 'inherit-only', label: 'Solo herencia (no aplicar a esta)', flags: ['f', 'd', 'i'] },
]

// ─── 4. ARBOL DE PERMISOS ────────────────────────────────────────────────────
interface PermLeaf {
  char: string
  label: string
  checked: boolean
}
interface PermGroup {
  label: string
  children: PermLeaf[]
}

const permGroups = reactive<PermGroup[]>([
  {
    label: 'Lectura',
    children: [
      { char: 'r', label: 'Lista de carpetas / Leer datos', checked: false },
      { char: 'x', label: 'Recorrer carpetas / Ejecutar', checked: false },
      { char: 'R', label: 'Leer atributos', checked: false },
      { char: 'a', label: 'Leer atributos extendidos', checked: false },
      { char: 'c', label: 'Leer permisos', checked: false },
    ],
  },
  {
    label: 'Escritura',
    children: [
      { char: 'w', label: 'Crear archivos / Escribir datos', checked: false },
      { char: 'p', label: 'Crear carpetas / Anexar datos', checked: false },
      { char: 'd', label: 'Eliminar', checked: false },
      { char: 'D', label: 'Eliminar subcarpetas y archivos', checked: false },
      { char: 'A', label: 'Escribir atributos', checked: false },
      { char: 'W', label: 'Escribir atributos extendidos', checked: false },
    ],
  },
  {
    label: 'Administracion',
    children: [
      { char: 'C', label: 'Cambiar permisos', checked: false },
      { char: 'o', label: 'Toma de posesion', checked: false },
      { char: 's', label: 'Sincronizar', checked: false },
    ],
  },
])

// Directiva custom para indeterminate (propiedad DOM no reactiva en Vue)
const vIndeterminate = {
  mounted(el: HTMLInputElement, binding: { value: boolean }) {
    el.indeterminate = binding.value
  },
  updated(el: HTMLInputElement, binding: { value: boolean }) {
    el.indeterminate = binding.value
  },
}

// Calcula estado del padre desde los hijos (logica ascendente)
function groupState(group: PermGroup): { checked: boolean; indeterminate: boolean } {
  const allChecked = group.children.every((c) => c.checked)
  const noneChecked = group.children.every((c) => !c.checked)
  return {
    checked: allChecked,
    indeterminate: !allChecked && !noneChecked,
  }
}

// Logica descendente: marcar/desmarcar padre -> todos los hijos al mismo estado
function toggleGroup(group: PermGroup, checked: boolean) {
  group.children.forEach((c) => {
    c.checked = checked
  })
}

// Logica ascendente: al cambiar un hijo, el padre se recalcula via groupState()
// (no necesita mutacion manual, es reactivo via computed en el template)

// ─── 5. PROCESAMIENTO ────────────────────────────────────────────────────────
const error = ref('')

function reset() {
  selectedSubjects.value = []
  subjectInput.value = ''
  subjectType.value = 'user'
  showDropdown.value = false
  inheritFrom.value = 'Ninguno'
  ruleType.value = 'allow'
  applyTo.value = 'all'
  permGroups.forEach((g) => g.children.forEach((c) => (c.checked = false)))
  error.value = ''
}

function cancel() {
  reset()
  emit('cancel')
}

function done() {
  error.value = ''
  if (selectedSubjects.value.length === 0) {
    error.value = 'Debes seleccionar al menos un usuario o grupo'
    return
  }

  const permissions: Record<string, boolean> = {}
  permGroups.forEach((g) => {
    g.children.forEach((c) => {
      permissions[c.char] = c.checked
    })
  })

  const applyOpt = applyToOptions.find((o) => o.value === applyTo.value)

  const config: AclConfig = {
    subjects: [...selectedSubjects.value],
    inheritFrom: inheritFrom.value,
    type: ruleType.value,
    applyTo: applyTo.value,
    permissions,
    flags: applyOpt?.flags ?? ['f', 'd'],
  }

  emit('done', config)
}

// Exponer para debug / tests
defineExpose({ reset, permGroups, selectedSubjects })
</script>

<template>
  <div class="permission-editor">
    <!-- Seccion combinada: Sujetos + Reglas + Herencia -->
    <div class="card compact-card">
      <!-- Fila 1: Sujetos -->
      <div class="compact-section">
        <div class="compact-label">Usuarios / Grupos</div>
        <div class="compact-controls">
          <select v-model="subjectType" class="select-mini select-type">
            <option value="user">Usuario</option>
            <option value="group">Grupo</option>
          </select>
          <div style="position: relative; flex: 1; min-width: 0;">
            <input
              v-model="subjectInput"
              @focus="showDropdown = true"
              @blur="onInputBlur"
              @keydown="onSubjectInputKeydown"
              :placeholder="subjectType === 'group' ? 'Buscar grupo...' : 'Buscar usuario...'"
              class="input-mini"
            />
            <div v-if="showDropdown && filteredOptions.length > 0" class="dropdown-mini">
              <div
                v-for="opt in filteredOptions"
                :key="opt"
                @mousedown.prevent="addSubject(opt)"
                class="dropdown-item"
              >{{ opt }}</div>
            </div>
          </div>
        </div>
        <div v-if="selectedSubjects.length > 0" class="tags-row">
          <span v-for="(s, i) in selectedSubjects" :key="s.type + ':' + s.name" class="tag">
            <span class="tag-type" :class="s.type">{{ s.type === 'group' ? 'G' : 'U' }}</span>
            {{ s.name }}
            <button class="tag-remove" @click="removeSubject(i)">&times;</button>
          </span>
        </div>
      </div>

      <div class="compact-divider" />

      <!-- Fila 2: Reglas + Herencia en una linea -->
      <div class="compact-section">
        <div class="compact-row-3">
          <div class="compact-field">
            <div class="compact-label">Tipo</div>
            <select v-model="ruleType" class="select-mini">
              <option value="allow">Permitir</option>
              <option value="deny">Denegar</option>
            </select>
          </div>
          <div class="compact-field">
            <div class="compact-label">Aplicar a</div>
            <select v-model="applyTo" class="select-mini">
              <option v-for="opt in applyToOptions" :key="opt.value" :value="opt.value">{{ opt.label }}</option>
            </select>
          </div>
          <div class="compact-field">
            <div class="compact-label">Heredar de</div>
            <select v-model="inheritFrom" class="select-mini">
              <option v-for="opt in inheritOptions" :key="opt" :value="opt">{{ opt }}</option>
            </select>
          </div>
        </div>
        <div v-if="permsDisabled" class="alert alert-warning compact-alert">
          Permisos deshabilitados (heredados de "{{ inheritFrom }}")
        </div>
      </div>
    </div>

    <!-- Arbol de permisos compacto -->
    <div class="card compact-card">
      <div class="compact-label" style="margin-bottom: 6px;">Permisos</div>
      <div class="perm-grid-3col">
        <div v-for="group in permGroups" :key="group.label" class="perm-group-compact">
          <div class="perm-group-header-compact">
            <input
              type="checkbox"
              class="tree-checkbox"
              :checked="groupState(group).checked"
              v-indeterminate="groupState(group).indeterminate"
              :disabled="permsDisabled"
              @change="toggleGroup(group, ($event.target as HTMLInputElement).checked)"
            />
            <span class="perm-group-label-compact">{{ group.label }}</span>
          </div>
          <div class="perm-group-children-compact">
            <label
              v-for="child in group.children"
              :key="child.char"
              class="perm-leaf-compact"
              :class="{ disabled: permsDisabled }"
            >
              <input
                type="checkbox"
                class="tree-checkbox"
                v-model="child.checked"
                :disabled="permsDisabled"
              />
              <span class="perm-leaf-char">{{ child.char }}</span>
              <span class="perm-leaf-label-compact">{{ child.label }}</span>
            </label>
          </div>
        </div>
      </div>
    </div>

    <!-- Error + botones -->
    <div v-if="error" class="alert alert-error compact-alert">{{ error }}</div>
    <div class="card compact-card compact-actions">
      <button class="btn btn-secondary" @click="cancel">Cancelar</button>
      <button class="btn btn-primary" @click="done">Finalizado</button>
    </div>
  </div>
</template>

<style scoped>
.compact-card {
  padding: 12px 14px !important;
  margin-bottom: 10px !important;
}

.compact-section {
  padding: 2px 0;
}

.compact-divider {
  height: 1px;
  background: var(--border);
  margin: 8px 0;
}

.compact-label {
  font-size: 11px;
  color: var(--text-secondary);
  margin-bottom: 3px;
  text-transform: uppercase;
  letter-spacing: 0.5px;
}

.compact-controls {
  display: flex;
  gap: 6px;
  align-items: flex-end;
}

.compact-row-3 {
  display: grid;
  grid-template-columns: 1fr 2fr 1.5fr;
  gap: 8px;
}

.compact-field {
  display: flex;
  flex-direction: column;
}

.select-mini,
.input-mini {
  padding: 5px 8px;
  font-size: 12px;
  background: var(--bg-input);
  border: 1px solid var(--border);
  border-radius: 5px;
  color: var(--text-primary);
  outline: none;
  width: 100%;
}

.select-type {
  flex: 0 0 90px;
  width: 90px;
}

.select-mini:focus,
.input-mini:focus {
  border-color: var(--accent);
}

.compact-alert {
  padding: 6px 10px !important;
  font-size: 11px !important;
  margin-top: 6px !important;
  margin-bottom: 0 !important;
}

.dropdown-mini {
  position: absolute;
  top: 100%;
  left: 0;
  right: 0;
  background: var(--bg-input);
  border: 1px solid var(--border);
  border-radius: 5px;
  max-height: 160px;
  overflow-y: auto;
  z-index: 10;
}

.dropdown-item {
  padding: 5px 8px;
  cursor: pointer;
  font-size: 12px;
}

.dropdown-item:hover {
  background: var(--bg-hover);
  color: var(--accent);
}

.tags-row {
  display: flex;
  flex-wrap: wrap;
  gap: 4px;
  margin-top: 6px;
}

.tag {
  display: inline-flex;
  align-items: center;
  gap: 3px;
  background: var(--bg-tertiary);
  border: 1px solid var(--border);
  border-radius: 12px;
  padding: 2px 8px;
  font-size: 11px;
}

.tag-type {
  display: inline-flex;
  align-items: center;
  justify-content: center;
  width: 16px;
  height: 16px;
  border-radius: 50%;
  font-size: 9px;
  font-weight: 700;
}

.tag-type.user {
  background: rgba(78, 154, 241, 0.25);
  color: var(--accent);
}

.tag-type.group {
  background: rgba(46, 204, 113, 0.25);
  color: var(--success);
}

.tag-remove {
  background: none;
  border: none;
  color: var(--text-muted);
  cursor: pointer;
  font-size: 14px;
  padding: 0 1px;
  line-height: 1;
}

.tag-remove:hover {
  color: var(--danger);
}

.perm-grid-3col {
  display: grid;
  grid-template-columns: repeat(3, 1fr);
  gap: 8px;
}

.perm-group-compact {
  border: 1px solid var(--border);
  border-radius: 6px;
  padding: 6px 8px;
}

.perm-group-header-compact {
  display: flex;
  align-items: center;
  gap: 6px;
  padding-bottom: 4px;
  border-bottom: 1px solid var(--border);
  margin-bottom: 4px;
}

.perm-group-label-compact {
  font-weight: 600;
  font-size: 12px;
}

.perm-group-children-compact {
  display: flex;
  flex-direction: column;
  gap: 2px;
}

.perm-leaf-compact {
  display: flex;
  align-items: center;
  gap: 5px;
  padding: 2px 4px;
  border-radius: 3px;
  cursor: pointer;
  font-size: 11px;
}

.perm-leaf-compact:hover {
  background: var(--bg-hover);
}

.perm-leaf-compact.disabled {
  opacity: 0.4;
  cursor: not-allowed;
}

.perm-leaf-char {
  font-family: monospace;
  font-weight: 700;
  color: var(--accent);
  width: 12px;
  text-align: center;
  font-size: 11px;
}

.perm-leaf-label-compact {
  color: var(--text-secondary);
}

.compact-actions {
  display: flex;
  gap: 8px;
  justify-content: flex-end;
}
</style>
