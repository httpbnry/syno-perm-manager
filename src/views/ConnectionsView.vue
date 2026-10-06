<script setup lang="ts">
import { ref, onMounted, onBeforeUnmount } from 'vue'
import { useRouter } from 'vue-router'
import { useConnectionStore } from '../stores/connection'
import type { ConnectionInput } from '../types'

const router = useRouter()
const store = useConnectionStore()

const showForm = ref(false)
const testing = ref(false)
const connecting = ref<number | null>(null)
const testResult = ref<string>('')
const testError = ref<string>('')
let disposed = false
onBeforeUnmount(() => { disposed = true })

const form = ref<ConnectionInput>({
  name: '',
  host: '',
  port: 22,
  username: '',
  password: '',
  use_sudo: true,
  auth_method: 'password',
  key_path: null,
  key_passphrase: null,
})

onMounted(() => {
  store.loadConnections()
})

function toggleForm() {
  showForm.value = !showForm.value
  if (!showForm.value) {
    form.value = { name: '', host: '', port: 22, username: '', password: '', use_sudo: true, auth_method: 'password', key_path: null, key_passphrase: null }
    testResult.value = ''
    testError.value = ''
  }
}

async function testConn() {
  testing.value = true
  testResult.value = ''
  testError.value = ''
  try {
    testResult.value = await store.testConnection(
      form.value.host,
      form.value.port,
      form.value.username,
      form.value.password,
      form.value.use_sudo,
      form.value.auth_method,
      form.value.key_path,
      form.value.key_passphrase,
    )
  } catch (e: any) {
    testError.value = String(e)
  } finally {
    testing.value = false
  }
}

async function save() {
  const ok = await store.saveConnection(form.value)
  if (ok) {
    toggleForm()
  }
}

async function connect(id: number) {
  connecting.value = id
  const ok = await store.connect(id)
  connecting.value = null
  if (ok && !disposed) {
    router.push('/explorer')
  }
}

async function remove(id: number) {
  if (confirm('Eliminar esta conexion?')) {
    await store.deleteConnection(id)
  }
}
</script>

<template>
  <div class="view-header">
    <h1>Conexiones NAS</h1>
    <p>Gestiona las conexiones SSH a tus dispositivos Synology</p>
  </div>

  <div v-if="store.error" class="alert alert-error">{{ store.error }}</div>
  <div v-if="testResult" class="alert alert-success">Conexion exitosa: {{ testResult }}</div>
  <div v-if="testError" class="alert alert-error">{{ testError }}</div>

  <div class="card" v-if="showForm">
    <div class="card-title">Nueva conexion</div>
    <div class="form-row">
      <div class="form-group">
        <label>Nombre</label>
        <input v-model="form.name" placeholder="NAS Oficina" />
      </div>
      <div class="form-group">
        <label>Host / IP</label>
        <input v-model="form.host" placeholder="192.168.1.100" />
      </div>
    </div>
    <div class="form-row">
      <div class="form-group">
        <label>Puerto</label>
        <input v-model.number="form.port" type="number" />
      </div>
      <div class="form-group">
        <label>Usuario</label>
        <input v-model="form.username" placeholder="admin" />
      </div>
    </div>

    <!-- Selector metodo de autenticacion -->
    <div class="form-group">
      <label>Autenticacion</label>
      <select v-model="form.auth_method">
        <option value="password">Password</option>
        <option value="key">Clave SSH</option>
      </select>
    </div>

    <!-- Password -->
    <div v-if="form.auth_method === 'password'" class="form-group">
      <label>Contraseña</label>
      <input v-model="form.password" type="password" placeholder="Se guarda en el keyring del SO" />
    </div>

    <!-- SSH Key -->
    <div v-if="form.auth_method === 'key'">
      <div class="form-group">
        <label>Ruta de la clave SSH</label>
        <input v-model="form.key_path" placeholder="C:\Users\TU\.ssh\id_ed25519" />
      </div>
      <div class="form-group">
        <label>Passphrase de la clave (opcional)</label>
        <input v-model="form.key_passphrase" type="password" placeholder="Si la clave tiene passphrase" />
      </div>
      <div class="alert alert-warning" style="font-size: 11px; margin-bottom: 10px;">
        La passphrase se guarda en el keyring. La clave SSH NO se copia, solo se guarda la ruta.
      </div>
    </div>

    <div class="form-group">
      <div class="form-check">
        <input type="checkbox" v-model="form.use_sudo" />
        <label style="margin: 0">Usar sudo (necesario para synoacltool)</label>
      </div>
    </div>
    <div class="btn-group">
      <button class="btn btn-secondary" @click="testConn" :disabled="testing">
        <span v-if="testing" class="loading-spinner"></span>
        Probar conexion
      </button>
      <button class="btn btn-primary" @click="save">Guardar</button>
      <button class="btn btn-secondary" @click="toggleForm">Cancelar</button>
    </div>
  </div>

  <div class="card" v-if="!showForm">
    <div style="display: flex; justify-content: space-between; align-items: center;">
      <div class="card-title" style="margin: 0">Conexiones guardadas</div>
      <button class="btn btn-primary btn-sm" @click="toggleForm">+ Nueva</button>
    </div>
  </div>

  <div class="card" v-if="store.connections.length > 0">
    <table>
      <thead>
        <tr>
          <th>Nombre</th>
          <th>Host</th>
          <th>Usuario</th>
          <th>Auth</th>
          <th>Sudo</th>
          <th>Acciones</th>
        </tr>
      </thead>
      <tbody>
        <tr v-for="c in store.connections" :key="c.id ?? 0">
          <td>{{ c.name }}</td>
          <td>{{ c.host }}</td>
          <td>{{ c.username }}</td>
          <td>
            <span class="badge" :class="c.auth_method === 'key' ? 'badge-info' : 'badge-success'">{{ c.auth_method === 'key' ? 'SSH Key' : 'Password' }}</span>
          </td>
          <td>
            <span v-if="c.use_sudo" class="badge badge-info">Si</span>
            <span v-else class="badge badge-warning">No</span>
          </td>
          <td>
            <div class="btn-group">
              <button class="btn btn-primary btn-sm" @click="connect(c.id!)" :disabled="store.connecting">
                <span v-if="connecting === c.id" class="loading-spinner"></span>
                <span v-if="connecting === c.id"> Conectando...</span>
                <span v-else>Conectar</span>
              </button>
              <button class="btn btn-danger btn-sm" @click="remove(c.id!)">Eliminar</button>
              <button class="btn btn-secondary btn-sm" @click="router.push({ name: 'settings', query: { connection: c.id } })">Ajustes NAS</button>
            </div>
          </td>
        </tr>
      </tbody>
    </table>
  </div>

  <div class="empty-state" v-if="store.connections.length === 0 && !showForm">
    <p>No hay conexiones guardadas. Crea una con el boton "+ Nueva".</p>
  </div>
</template>
