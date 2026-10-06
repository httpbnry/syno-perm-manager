<script setup lang="ts">
import { ref, computed, onMounted } from 'vue'
import { invoke } from '@tauri-apps/api/core'
import { useRouter } from 'vue-router'
import { generatePassword } from '../utils/password'
import { useExplorerStore } from '../stores/explorer'
import type { UserDetail, GroupDetail, CreateUserInput, CreateGroupInput } from '../types'

const explorerStore = useExplorerStore()
const router = useRouter()

const activeTab = ref<'users' | 'groups'>('users')
const searchQuery = ref('')
const loadingDetail = ref(false)
const error = ref('')
const successMsg = ref('')

const selectedUser = ref<UserDetail | null>(null)
const selectedGroup = ref<GroupDetail | null>(null)

const showCreateUser = ref(false)
const showCreateGroup = ref(false)
const showSetPassword = ref(false)
const showAddMember = ref(false)
const passwordTarget = ref('')
const newPassword = ref('')
const newMemberName = ref('')
let detailVersion = 0

const newUserForm = ref<CreateUserInput>({
  username: '', password: '', full_name: '', expired: false, mail: '', privilege: 0,
})
const newGroupForm = ref<CreateGroupInput>({ name: '', members: [] })

const autoFullName = ref('')

function generateUsername(fullName: string): string {
  const parts = fullName.trim().split(/\s+/)
  if (parts.length < 2) return ''
  const firstInitial = parts[0].charAt(0).toUpperCase()
  const lastName = parts[parts.length - 1]
  return firstInitial + lastName.charAt(0).toUpperCase() + lastName.slice(1).toLowerCase()
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

  newUserForm.value.username = generateUsername(fullName)

  if (!newUserForm.value.password) {
    newUserForm.value.password = generatePassword()
  }

  newUserForm.value.full_name = `${fullName} Alta ${todayDDMMYY()}`
}

function regenPassword() {
  newUserForm.value.password = generatePassword()
  if (autoFullName.value.trim()) {
    const fullName = autoFullName.value.trim()
    newUserForm.value.full_name = `${fullName} Alta ${todayDDMMYY()}`
  }
}

const filteredUsers = computed(() => {
  const q = searchQuery.value.toLowerCase()
  if (!q) return explorerStore.users
  return explorerStore.users.filter((u) => u.toLowerCase().includes(q))
})

const filteredGroups = computed(() => {
  const q = searchQuery.value.toLowerCase()
  if (!q) return explorerStore.groups
  return explorerStore.groups.filter((g) => g.toLowerCase().includes(q))
})

onMounted(() => {
  explorerStore.loadUsersGroups()
})

async function selectUser(name: string) {
  const version = ++detailVersion
  loadingDetail.value = true
  selectedGroup.value = null
  error.value = ''
  try {
    const detail = await invoke<UserDetail>('get_user_detail', { username: name })
    if (version === detailVersion) selectedUser.value = detail
  } catch (e: any) {
    error.value = String(e)
  } finally {
    if (version === detailVersion) loadingDetail.value = false
  }
}

async function selectGroup(name: string) {
  const version = ++detailVersion
  loadingDetail.value = true
  selectedUser.value = null
  error.value = ''
  try {
    const detail = await invoke<GroupDetail>('get_group_detail', { groupname: name })
    if (version === detailVersion) selectedGroup.value = detail
  } catch (e: any) {
    error.value = String(e)
  } finally {
    if (version === detailVersion) loadingDetail.value = false
  }
}

async function createUser() {
  error.value = ''
  successMsg.value = ''
  try {
    await invoke('create_user', { input: newUserForm.value })
    successMsg.value = `Usuario ${newUserForm.value.username} creado. Password: ${newUserForm.value.password}`
    showCreateUser.value = false
    autoFullName.value = ''
    newUserForm.value = { username: '', password: '', full_name: '', expired: false, mail: '', privilege: 0 }
    await explorerStore.loadUsersGroups(true)
  } catch (e: any) {
    error.value = String(e)
  }
}

async function removeUser(name: string) {
  if (!confirm(`Eliminar usuario ${name}?`)) return
  error.value = ''
  successMsg.value = ''
  try {
    await invoke('delete_user', { usernames: [name] })
    successMsg.value = `Usuario ${name} eliminado`
    selectedUser.value = null
    await explorerStore.loadUsersGroups(true)
  } catch (e: any) {
    error.value = String(e)
  }
}

async function changePassword() {
  error.value = ''
  successMsg.value = ''
  try {
    await invoke('set_user_password', { username: passwordTarget.value, password: newPassword.value })
    successMsg.value = `Password de ${passwordTarget.value} cambiado`
    showSetPassword.value = false
    newPassword.value = ''
  } catch (e: any) {
    error.value = String(e)
  }
}

async function renameUser(oldName: string) {
  const newName = prompt(`Renombrar usuario ${oldName} a:`, oldName)
  if (!newName || newName === oldName) return
  error.value = ''
  successMsg.value = ''
  try {
    await invoke('rename_user', { oldName, newName })
    successMsg.value = `Usuario renombrado a ${newName}`
    selectedUser.value = null
    await explorerStore.loadUsersGroups(true)
  } catch (e: any) {
    error.value = String(e)
  }
}

async function createGroup() {
  error.value = ''
  successMsg.value = ''
  try {
    await invoke('create_group', { input: newGroupForm.value })
    successMsg.value = `Grupo ${newGroupForm.value.name} creado`
    showCreateGroup.value = false
    newGroupForm.value = { name: '', members: [] }
    await explorerStore.loadUsersGroups(true)
  } catch (e: any) {
    error.value = String(e)
  }
}

async function removeGroup(name: string) {
  if (!confirm(`Eliminar grupo ${name}?`)) return
  error.value = ''
  successMsg.value = ''
  try {
    await invoke('delete_group', { groupnames: [name] })
    successMsg.value = `Grupo ${name} eliminado`
    selectedGroup.value = null
    await explorerStore.loadUsersGroups(true)
  } catch (e: any) {
    error.value = String(e)
  }
}

async function addMember() {
  if (!selectedGroup.value || !newMemberName.value) return
  error.value = ''
  successMsg.value = ''
  try {
    await invoke('add_group_member', { groupname: selectedGroup.value.name, username: newMemberName.value })
    successMsg.value = `${newMemberName.value} anadido a ${selectedGroup.value.name}`
    newMemberName.value = ''
    showAddMember.value = false
    await selectGroup(selectedGroup.value.name)
  } catch (e: any) {
    error.value = String(e)
  }
}

async function removeMember(member: string) {
  if (!selectedGroup.value) return
  if (!confirm(`Quitar a ${member} del grupo ${selectedGroup.value.name}?`)) return
  error.value = ''
  successMsg.value = ''
  const remaining = selectedGroup.value.members.filter((m) => m !== member)
  try {
    await invoke('set_group_members', { groupname: selectedGroup.value.name, members: remaining })
    successMsg.value = `${member} quitado de ${selectedGroup.value.name}`
    await selectGroup(selectedGroup.value.name)
  } catch (e: any) {
    error.value = String(e)
  }
}

async function renameGroup(oldName: string) {
  const newName = prompt(`Renombrar grupo ${oldName} a:`, oldName)
  if (!newName || newName === oldName) return
  error.value = ''
  successMsg.value = ''
  try {
    await invoke('rename_group', { oldName, newName })
    successMsg.value = `Grupo renombrado a ${newName}`
    selectedGroup.value = null
    await explorerStore.loadUsersGroups(true)
  } catch (e: any) {
    error.value = String(e)
  }
}

function openSetPassword(name: string) {
  passwordTarget.value = name
  newPassword.value = ''
  showSetPassword.value = true
}
</script>

<template>
  <div class="view-header">
    <h1>Gestor de usuarios y grupos</h1>
    <p>Administra usuarios y grupos del NAS</p>
  </div>

  <div v-if="error" class="alert alert-error">{{ error }}</div>
  <div v-if="successMsg" class="alert alert-success">{{ successMsg }}</div>

  <!-- Tabs -->
  <div class="card" style="padding: 0 14px 10px;">
    <div style="display: flex; gap: 4px; padding: 10px 0;">
      <button class="btn btn-sm" :class="activeTab === 'users' ? 'btn-primary' : 'btn-secondary'" @click="activeTab = 'users'">
        Usuarios ({{ explorerStore.users.length }})
      </button>
      <button class="btn btn-sm" :class="activeTab === 'groups' ? 'btn-primary' : 'btn-secondary'" @click="activeTab = 'groups'">
        Grupos ({{ explorerStore.groups.length }})
      </button>
      <div style="flex: 1;" />
      <input v-model="searchQuery" placeholder="Buscar..." style="width: 180px; padding: 5px 8px; font-size: 12px; background: var(--bg-input); border: 1px solid var(--border); border-radius: 5px; color: var(--text-primary);" />
      <button v-if="activeTab === 'users'" class="btn btn-primary btn-sm" @click="showCreateUser = true">+ Nuevo</button>
      <button v-else class="btn btn-primary btn-sm" @click="showCreateGroup = true">+ Nuevo</button>
    </div>
  </div>

  <!-- Layout: lista + detalle -->
  <div style="display: flex; gap: 12px; align-items: flex-start;">
    <!-- Lista -->
    <div class="card" style="flex: 0 0 280px; padding: 8px;">
      <div class="scrollable" style="max-height: 500px; border: none; padding: 0;">
        <div
          v-for="item in (activeTab === 'users' ? filteredUsers : filteredGroups)"
          :key="item"
          class="list-item"
          :class="{
            'list-item-active': (activeTab === 'users' && selectedUser?.name === item) || (activeTab === 'groups' && selectedGroup?.name === item)
          }"
          @click="activeTab === 'users' ? selectUser(item) : selectGroup(item)"
        >
          <span class="principal-type" :class="activeTab === 'users' ? 'user' : 'group'">{{ activeTab === 'users' ? 'U' : 'G' }}</span>
          {{ item }}
        </div>
      </div>
    </div>

    <!-- Detalle -->
    <div class="card" style="flex: 1; min-height: 300px;">
      <div v-if="loadingDetail" class="empty-state">
        <span class="loading-spinner"></span>
        <p>Cargando...</p>
      </div>

      <!-- Detalle Usuario -->
      <div v-else-if="selectedUser">
        <div style="display: flex; justify-content: space-between; align-items: center; margin-bottom: 16px;">
          <div class="card-title" style="margin: 0;">{{ selectedUser.name }}</div>
          <div class="btn-group">
            <button class="btn btn-secondary btn-sm" @click="openSetPassword(selectedUser.name)">Cambiar password</button>
            <button class="btn btn-primary btn-sm" @click="router.push({ name: 'compare', query: { target: selectedUser.name } })">Comparar permisos</button>
            <button class="btn btn-secondary btn-sm" @click="renameUser(selectedUser.name)">Renombrar</button>
            <button class="btn btn-danger btn-sm" @click="removeUser(selectedUser.name)">Eliminar</button>
          </div>
        </div>
        <table>
          <tbody>
            <tr><td style="color: var(--text-secondary); width: 140px;">UID</td><td style="font-family: monospace;">{{ selectedUser.uid }}</td></tr>
            <tr><td style="color: var(--text-secondary);">GID primario</td><td style="font-family: monospace;">{{ selectedUser.primary_gid }}</td></tr>
            <tr><td style="color: var(--text-secondary);">Nombre completo</td><td>{{ selectedUser.full_name || '-' }}</td></tr>
            <tr><td style="color: var(--text-secondary);">Email</td><td>{{ selectedUser.mail || '-' }}</td></tr>
            <tr><td style="color: var(--text-secondary);">Home</td><td style="font-family: monospace; font-size: 11px;">{{ selectedUser.user_dir }}</td></tr>
            <tr><td style="color: var(--text-secondary);">Shell</td><td style="font-family: monospace; font-size: 11px;">{{ selectedUser.shell }}</td></tr>
            <tr><td style="color: var(--text-secondary);">Expirado</td><td><span class="badge" :class="selectedUser.expired ? 'badge-danger' : 'badge-success'">{{ selectedUser.expired ? 'Si' : 'No' }}</span></td></tr>
            <tr><td style="color: var(--text-secondary);">Miembro de</td><td>
              <div style="display: flex; flex-wrap: wrap; gap: 4px;">
                <span v-for="g in selectedUser.member_of" :key="g" class="badge badge-info">{{ g }}</span>
                <span v-if="selectedUser.member_of.length === 0" style="color: var(--text-muted);">-</span>
              </div>
            </td></tr>
          </tbody>
        </table>
      </div>

      <!-- Detalle Grupo -->
      <div v-else-if="selectedGroup">
        <div style="display: flex; justify-content: space-between; align-items: center; margin-bottom: 16px;">
          <div class="card-title" style="margin: 0;">{{ selectedGroup.name }}</div>
          <div class="btn-group">
            <button class="btn btn-secondary btn-sm" @click="showAddMember = true">+ Miembro</button>
            <button class="btn btn-secondary btn-sm" @click="renameGroup(selectedGroup.name)">Renombrar</button>
            <button class="btn btn-danger btn-sm" @click="removeGroup(selectedGroup.name)">Eliminar</button>
          </div>
        </div>
        <table>
          <tbody>
            <tr><td style="color: var(--text-secondary); width: 140px;">GID</td><td style="font-family: monospace;">{{ selectedGroup.gid }}</td></tr>
            <tr><td style="color: var(--text-secondary);">Tipo</td><td>{{ selectedGroup.group_type }}</td></tr>
            <tr><td style="color: var(--text-secondary);">Descripcion</td><td>{{ selectedGroup.description || '-' }}</td></tr>
          </tbody>
        </table>
        <div style="margin-top: 16px;">
          <div class="compact-label" style="margin-bottom: 8px;">Miembros ({{ selectedGroup.members.length }})</div>
          <div style="display: flex; flex-wrap: wrap; gap: 6px;">
            <span v-for="m in selectedGroup.members" :key="m" class="member-chip">
              <span class="principal-type user">U</span>
              {{ m }}
              <button class="tag-remove" @click="removeMember(m)">&times;</button>
            </span>
            <span v-if="selectedGroup.members.length === 0" style="color: var(--text-muted); font-size: 12px;">Sin miembros</span>
          </div>
        </div>
      </div>

      <div v-else class="empty-state">
        <p>Selecciona un {{ activeTab === 'users' ? 'usuario' : 'grupo' }} de la lista</p>
      </div>
    </div>
  </div>

  <!-- Modal: Crear usuario -->
  <div v-if="showCreateUser" class="modal-overlay" @click.self="showCreateUser = false">
    <div class="card modal-card">
      <div class="card-title">Nuevo usuario</div>

      <div class="form-group">
        <label>Nombre completo (escribe y se autogenera todo)</label>
        <input
          v-model="autoFullName"
          @input="onFullNameInput"
          placeholder="Luis Gonzalez"
          autofocus
        />
      </div>

      <div class="form-row">
        <div class="form-group">
          <label>Usuario</label>
          <input v-model="newUserForm.username" placeholder="LGonzalez" />
        </div>
        <div class="form-group" style="flex: 0 0 40px;">
          <label>&nbsp;</label>
          <button class="btn btn-secondary btn-sm" @click="regenPassword" title="Generar nueva password" style="height: 33px;">&#8635;</button>
        </div>
        <div class="form-group">
          <label>Password</label>
          <input v-model="newUserForm.password" />
        </div>
      </div>

      <div class="form-group">
        <label>Descripcion</label>
        <input v-model="newUserForm.full_name" />
      </div>

      <div class="form-group">
        <label>Email</label>
        <input v-model="newUserForm.mail" placeholder="usuario@empresa.com" />
      </div>

      <div class="form-group">
        <label>Privilegio (0=normal, 1=admin)</label>
        <input v-model.number="newUserForm.privilege" type="number" min="0" max="1" />
      </div>

      <div class="create-preview" v-if="newUserForm.username">
        <div>Se creara: <strong>{{ newUserForm.username }}</strong> / <code>{{ newUserForm.password }}</code></div>
      </div>

      <div class="btn-group" style="margin-top: 12px;">
        <button class="btn btn-primary" @click="createUser">Crear</button>
        <button class="btn btn-secondary" @click="showCreateUser = false">Cancelar</button>
      </div>
    </div>
  </div>

  <!-- Modal: Crear grupo -->
  <div v-if="showCreateGroup" class="modal-overlay" @click.self="showCreateGroup = false">
    <div class="card modal-card">
      <div class="card-title">Nuevo grupo</div>
      <div class="form-group">
        <label>Nombre del grupo</label>
        <input v-model="newGroupForm.name" placeholder="MARKETING" />
      </div>
      <div class="form-group">
        <label>Miembros (separados por coma)</label>
        <input
          :value="newGroupForm.members.join(', ')"
          @input="newGroupForm.members = ($event.target as HTMLInputElement).value.split(',').map((s: string) => s.trim()).filter((s: string) => s)"
          placeholder="jperez, mgarcia"
        />
      </div>
      <div class="btn-group" style="margin-top: 12px;">
        <button class="btn btn-primary" @click="createGroup">Crear</button>
        <button class="btn btn-secondary" @click="showCreateGroup = false">Cancelar</button>
      </div>
    </div>
  </div>

  <!-- Modal: Cambiar password -->
  <div v-if="showSetPassword" class="modal-overlay" @click.self="showSetPassword = false">
    <div class="card modal-card">
      <div class="card-title">Cambiar password de {{ passwordTarget }}</div>
      <div class="form-group">
        <label>Nueva password</label>
        <input v-model="newPassword" type="password" />
      </div>
      <div class="btn-group" style="margin-top: 12px;">
        <button class="btn btn-primary" @click="changePassword">Cambiar</button>
        <button class="btn btn-secondary" @click="showSetPassword = false">Cancelar</button>
      </div>
    </div>
  </div>

  <!-- Modal: Anadir miembro -->
  <div v-if="showAddMember && selectedGroup" class="modal-overlay" @click.self="showAddMember = false">
    <div class="card modal-card">
      <div class="card-title">Anadir miembro a {{ selectedGroup.name }}</div>
      <div class="form-group">
        <label>Usuario</label>
        <select v-model="newMemberName">
          <option value="" disabled>Seleccionar...</option>
          <option v-for="u in explorerStore.users" :key="u" :value="u">{{ u }}</option>
        </select>
      </div>
      <div class="btn-group" style="margin-top: 12px;">
        <button class="btn btn-primary" @click="addMember">Anadir</button>
        <button class="btn btn-secondary" @click="showAddMember = false">Cancelar</button>
      </div>
    </div>
  </div>
</template>

<style scoped>
.list-item {
  display: flex;
  align-items: center;
  gap: 6px;
  padding: 6px 10px;
  border-radius: 5px;
  cursor: pointer;
  font-size: 12px;
  transition: background 0.1s;
}

.list-item:hover {
  background: var(--bg-hover);
}

.list-item-active {
  background: var(--bg-tertiary);
  color: var(--accent);
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

.member-chip {
  display: inline-flex;
  align-items: center;
  gap: 4px;
  background: var(--bg-tertiary);
  border: 1px solid var(--border);
  border-radius: 14px;
  padding: 3px 10px;
  font-size: 11px;
}

.tag-remove {
  background: none;
  border: none;
  color: var(--text-muted);
  cursor: pointer;
  font-size: 14px;
  padding: 0 1px;
}

.tag-remove:hover {
  color: var(--danger);
}

.modal-overlay {
  position: fixed;
  top: 0;
  left: 0;
  right: 0;
  bottom: 0;
  background: rgba(0, 0, 0, 0.6);
  display: flex;
  align-items: center;
  justify-content: center;
  z-index: 100;
}

.modal-card {
  width: 420px;
  max-width: 90vw;
  padding: 20px;
}

.compact-label {
  font-size: 11px;
  color: var(--text-secondary);
  text-transform: uppercase;
  letter-spacing: 0.5px;
}

.create-preview {
  background: var(--bg-input);
  border: 1px solid var(--border);
  border-radius: 6px;
  padding: 8px 12px;
  font-size: 12px;
  color: var(--text-secondary);
  margin-top: 4px;
}

.create-preview code {
  color: var(--accent);
  font-family: monospace;
}
</style>
