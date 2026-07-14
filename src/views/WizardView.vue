<script setup lang="ts">
import { ref, computed } from 'vue'
import { invoke } from '@tauri-apps/api/core'
import { useRouter } from 'vue-router'
import { useExplorerStore } from '../stores/explorer'
import type { CreateUserInput, AclEntry } from '../types'

const router = useRouter()
const store = useExplorerStore()

const step = ref(1)
const error = ref('')
const success = ref('')
const creating = ref(false)

const autoFullName = ref('')
const passwordGen = ref('')

const userForm = ref<CreateUserInput>({
  username: '', password: '', full_name: '', expired: false, mail: '', privilege: 0,
})

const selectedGroups = ref<string[]>([])
const selectedFolderPerms = ref<Map<string, AclEntry[]>>(new Map())

const PERM_PRESETS: Record<string, { perms: string; flags: string }> = {
  'Lectura': { perms: 'r-x---a-R-c--', flags: 'fd--' },
  'Escritura': { perms: 'rwxpdDaARWc--', flags: 'fd--' },
  'Sin acceso': { perms: '----------', flags: 'fd--' },
}

store.loadUsersGroups()
store.loadShares()

function generateUsername(fullName: string): string {
  const parts = fullName.trim().split(/\s+/)
  if (parts.length < 2) return ''
  const firstInitial = parts[0].charAt(0).toUpperCase()
  const lastName = parts[parts.length - 1]
  return firstInitial + lastName.charAt(0).toUpperCase() + lastName.slice(1).toLowerCase()
}

function generatePassword(length: number = 12): string {
  const chars = 'abcdefghijkmnopqrstuvwxyzABCDEFGHJKLMNPQRSTUVWXYZ23456789!@#%'
  let pwd = ''
  for (let i = 0; i < length; i++) {
    pwd += chars[Math.floor(Math.random() * chars.length)]
  }
  return pwd
}

function todayDDMMYY(): string {
  const d = new Date()
  const dd = String(d.getDate()).padStart(2, '0')
  const mm = String(d.getMonth() + 1).padStart(2, '0')
  const yy = String(d.getFullYear()).slice(2)
  return dd + mm + yy
}

function onFullNameInput() {
  const fullName = autoFullName.value.trim()
  if (!fullName) return
  userForm.value.username = generateUsername(fullName)
  if (!passwordGen.value) {
    passwordGen.value = generatePassword()
    userForm.value.password = passwordGen.value
  }
  userForm.value.full_name = `${fullName} Alta ${todayDDMMYY()} ${passwordGen.value}`
}

function regenPassword() {
  passwordGen.value = generatePassword()
  userForm.value.password = passwordGen.value
  if (autoFullName.value.trim()) {
    userForm.value.full_name = `${autoFullName.value.trim()} Alta ${todayDDMMYY()} ${passwordGen.value}`
  }
}

function toggleGroup(name: string) {
  const idx = selectedGroups.value.indexOf(name)
  if (idx >= 0) {
    selectedGroups.value.splice(idx, 1)
  } else {
    selectedGroups.value.push(name)
  }
}

function applyPresetToShare(path: string, preset: string) {
  const p = PERM_PRESETS[preset]
  const entry: AclEntry = {
    principal_type: 'user:allow',
    name: userForm.value.username,
    permissions: p.perms,
    flags: p.flags,
  }
  const current = selectedFolderPerms.value.get(path) || []
  const filtered = current.filter((e) => e.name !== userForm.value.username)
  if (preset !== 'Sin acceso' || filtered.length > 0) {
    filtered.push(entry)
  }
  selectedFolderPerms.value.set(path, filtered)
  selectedFolderPerms.value = new Map(selectedFolderPerms.value)
}

function removeFolderPerm(path: string) {
  selectedFolderPerms.value.delete(path)
  selectedFolderPerms.value = new Map(selectedFolderPerms.value)
}

const foldersWithPerms = computed(() => Array.from(selectedFolderPerms.value.keys()))

function nextStep() {
  error.value = ''
  if (step.value === 1) {
    if (!userForm.value.username || !userForm.value.password) {
      error.value = 'Completa el nombre y password'
      return
    }
    step.value = 2
  } else if (step.value === 2) {
    step.value = 3
  }
}

function prevStep() {
  error.value = ''
  if (step.value > 1) step.value--
}

async function finish() {
  creating.value = true
  error.value = ''
  success.value = ''

  try {
    await invoke('create_user', { input: userForm.value })

    for (const group of selectedGroups.value) {
      try {
        await invoke('add_group_member', { groupname: group, username: userForm.value.username })
      } catch (e: any) {
        console.error(`No se pudo anadir a ${group}:`, e)
      }
    }

    const paths: string[] = []
    const allEntries: AclEntry[] = []
    for (const [path, entries] of selectedFolderPerms.value) {
      paths.push(path)
      allEntries.push(...entries)
    }

    if (paths.length > 0 && allEntries.length > 0) {
      try {
        await invoke('apply_acl', {
          request: { paths, entries: allEntries, recursive: false }
        })
      } catch (e: any) {
        console.error('Error aplicando permisos:', e)
      }
    }

    success.value = `Usuario ${userForm.value.username} creado correctamente. Password: ${passwordGen.value}`
    step.value = 4
  } catch (e: any) {
    error.value = String(e)
  } finally {
    creating.value = false
  }
}

function restart() {
  step.value = 1
  autoFullName.value = ''
  passwordGen.value = ''
  userForm.value = { username: '', password: '', full_name: '', expired: false, mail: '', privilege: 0 }
  selectedGroups.value = []
  selectedFolderPerms.value = new Map()
  success.value = ''
  error.value = ''
}
</script>

<template>
  <div class="view-header">
    <h1>Alta de usuario</h1>
    <p>Crea un usuario, asignalo a grupos y dale permisos en un solo flujo</p>
  </div>

  <div v-if="error" class="alert alert-error">{{ error }}</div>
  <div v-if="success" class="alert alert-success">{{ success }}</div>

  <!-- Progress -->
  <div class="card" style="padding: 12px 14px;">
    <div class="wizard-steps">
      <div class="wizard-step" :class="{ active: step >= 1, done: step > 1 }">
        <span class="step-num">1</span>
        <span class="step-label">Datos</span>
      </div>
      <div class="wizard-line" :class="{ active: step > 1 }"></div>
      <div class="wizard-step" :class="{ active: step >= 2, done: step > 2 }">
        <span class="step-num">2</span>
        <span class="step-label">Grupos</span>
      </div>
      <div class="wizard-line" :class="{ active: step > 2 }"></div>
      <div class="wizard-step" :class="{ active: step >= 3, done: step > 3 }">
        <span class="step-num">3</span>
        <span class="step-label">Permisos</span>
      </div>
      <div class="wizard-line" :class="{ active: step > 3 }"></div>
      <div class="wizard-step" :class="{ active: step >= 4 }">
        <span class="step-num">&#10003;</span>
        <span class="step-label">Listo</span>
      </div>
    </div>
  </div>

  <!-- Step 1: Datos del usuario -->
  <div v-if="step === 1" class="card">
    <div class="card-title">Datos del usuario</div>
    <div class="form-group">
      <label>Nombre completo (autogenera todo)</label>
      <input v-model="autoFullName" @input="onFullNameInput" placeholder="Luis Gonzalez" autofocus />
    </div>
    <div class="form-row">
      <div class="form-group">
        <label>Usuario</label>
        <input v-model="userForm.username" placeholder="LGonzalez" />
      </div>
      <div class="form-group" style="flex: 0 0 40px;">
        <label>&nbsp;</label>
        <button class="btn btn-secondary btn-sm" @click="regenPassword" title="Nueva password" style="height: 33px;">&#8635;</button>
      </div>
      <div class="form-group">
        <label>Password</label>
        <input v-model="userForm.password" />
      </div>
    </div>
    <div class="form-group">
      <label>Descripcion</label>
      <input v-model="userForm.full_name" />
    </div>
    <div class="form-group">
      <label>Email</label>
      <input v-model="userForm.mail" placeholder="usuario@empresa.com" />
    </div>
    <div class="btn-group" style="margin-top: 12px;">
      <button class="btn btn-primary" @click="nextStep">Siguiente &rarr;</button>
    </div>
  </div>

  <!-- Step 2: Grupos -->
  <div v-if="step === 2" class="card">
    <div class="card-title">Asignar a grupos ({{ selectedGroups.length }} seleccionados)</div>
    <div class="scrollable check-list" style="max-height: 350px;">
      <label v-for="g in store.groups" :key="g" class="check-item">
        <input type="checkbox" :checked="selectedGroups.includes(g)" @change="toggleGroup(g)" />
        <span class="principal-type group">G</span>
        <span>{{ g }}</span>
      </label>
    </div>
    <div class="btn-group" style="margin-top: 12px;">
      <button class="btn btn-secondary" @click="prevStep">&larr; Atras</button>
      <button class="btn btn-primary" @click="nextStep">Siguiente &rarr;</button>
    </div>
  </div>

  <!-- Step 3: Permisos carpetas -->
  <div v-if="step === 3" class="card">
    <div class="card-title">Permisos en carpetas ({{ foldersWithPerms.length }} configuradas)</div>
    <div v-if="store.tree.length === 0" class="empty-state">
      <p>Cargando carpetas...</p>
    </div>
    <div v-else class="scrollable" style="max-height: 400px;">
      <div v-for="node in store.tree" :key="node.path" class="folder-perm-row">
        <span class="folder-icon">&#128193;</span>
        <span class="folder-name">{{ node.name }}</span>
        <div class="folder-actions">
          <button
            v-for="preset in Object.keys(PERM_PRESETS)"
            :key="preset"
            class="btn btn-sm"
            :class="selectedFolderPerms.get(node.path)?.some(e => e.permissions === PERM_PRESETS[preset].perms) ? 'btn-primary' : 'btn-secondary'"
            @click="applyPresetToShare(node.path, preset)"
          >{{ preset }}</button>
          <button
            v-if="selectedFolderPerms.has(node.path)"
            class="btn btn-danger btn-sm"
            @click="removeFolderPerm(node.path)"
          >Quitar</button>
        </div>
      </div>
    </div>
    <div class="btn-group" style="margin-top: 12px;">
      <button class="btn btn-secondary" @click="prevStep">&larr; Atras</button>
      <button class="btn btn-primary" @click="finish" :disabled="creating">
        <span v-if="creating" class="loading-spinner"></span>
        Crear usuario
      </button>
    </div>
  </div>

  <!-- Step 4: Done -->
  <div v-if="step === 4" class="card">
    <div class="empty-state" style="padding: 30px;">
      <div style="font-size: 48px; margin-bottom: 12px;">&#10004;</div>
      <h2 style="color: var(--success); margin-bottom: 8px;">Usuario creado</h2>
      <p style="margin-bottom: 4px;">Usuario: <strong>{{ userForm.username }}</strong></p>
      <p style="margin-bottom: 4px;">Password: <code style="color: var(--accent);">{{ passwordGen }}</code></p>
      <p style="margin-bottom: 4px;">Grupos: {{ selectedGroups.join(', ') || 'ninguno' }}</p>
      <p style="margin-bottom: 16px;">Carpetas: {{ foldersWithPerms.length }} configuradas</p>
      <div class="btn-group" style="justify-content: center;">
        <button class="btn btn-primary" @click="restart">Crear otro</button>
        <button class="btn btn-secondary" @click="router.push('/dashboard')">Dashboard</button>
      </div>
    </div>
  </div>
</template>

<style scoped>
.wizard-steps {
  display: flex;
  align-items: center;
  gap: 0;
}

.wizard-step {
  display: flex;
  align-items: center;
  gap: 6px;
  opacity: 0.4;
  transition: opacity 0.2s;
}

.wizard-step.active {
  opacity: 1;
}

.wizard-step.done {
  opacity: 0.7;
}

.step-num {
  display: inline-flex;
  align-items: center;
  justify-content: center;
  width: 24px;
  height: 24px;
  border-radius: 50%;
  background: var(--bg-tertiary);
  color: var(--text-secondary);
  font-size: 12px;
  font-weight: 700;
}

.wizard-step.active .step-num {
  background: var(--accent);
  color: white;
}

.wizard-step.done .step-num {
  background: var(--success);
  color: white;
}

.step-label {
  font-size: 12px;
  color: var(--text-secondary);
}

.wizard-line {
  flex: 1;
  height: 2px;
  background: var(--border);
  margin: 0 8px;
  transition: background 0.2s;
}

.wizard-line.active {
  background: var(--success);
}

.check-item {
  display: flex;
  align-items: center;
  gap: 6px;
  padding: 4px 8px;
  font-size: 12px;
  cursor: pointer;
  border-radius: 4px;
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

.principal-type.group {
  background: rgba(46, 204, 113, 0.25);
  color: var(--success);
}

.folder-perm-row {
  display: flex;
  align-items: center;
  gap: 8px;
  padding: 6px 8px;
  border-bottom: 1px solid var(--border);
}

.folder-icon {
  font-size: 14px;
}

.folder-name {
  font-size: 13px;
  flex: 1;
}

.folder-actions {
  display: flex;
  gap: 4px;
}
</style>
