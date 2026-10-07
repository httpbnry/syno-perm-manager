<script setup lang="ts">
import { ref, watch, onMounted, onBeforeUnmount } from 'vue'
import { invoke } from '@tauri-apps/api/core'
import { useConnectionStore } from '../stores/connection'
import type { StartupResult } from '../types'
export interface NasConfig {
  volume_path: string; synoacltool_path: string; synoshare_path: string; synouser_path: string; synogroup_path: string; find_path: string
  excluded_folders: string; ssh_timeout_secs: number; keepalive_secs: number; startup_script: string
}
const props = defineProps<{ initialId?: number }>()
const connection = useConnectionStore()
const id = ref<number | null>(props.initialId || connection.connectedId)
const config = ref<NasConfig | null>(null)
const loading = ref(false)
const saving = ref(false)
const error = ref('')
const message = ref('')
const testing = ref(false)
const test = ref<StartupResult | null>(null)
let version = 0
onBeforeUnmount(() => { version++ })
const fields = [
  ['volume_path', 'Volumen de referencia'], ['synoacltool_path', 'Binario synoacltool'], ['synoshare_path', 'Binario synoshare'],
  ['synouser_path', 'Binario synouser'], ['synogroup_path', 'Binario synogroup'], ['find_path', 'Binario find'], ['excluded_folders', 'Carpetas excluidas (separadas por comas)'],
] as const
async function load() {
  const current = ++version
  config.value = null; message.value = ''; error.value = ''; test.value = null
  if (id.value === null) return
  loading.value = true
  try {
    const data = await invoke<NasConfig>('get_nas_config', { id: id.value })
    if (current === version) config.value = data
  } catch (e) { if (current === version) error.value = String(e) }
  finally { if (current === version) loading.value = false }
}
watch(id, load)
onMounted(async () => {
  await connection.loadConnections()
  if (id.value === null) id.value = connection.connections[0]?.id ?? null
  else await load()
})
async function save() {
  if (!config.value || id.value === null || saving.value) return
  saving.value = true; error.value = ''; message.value = ''
  try {
    await invoke('save_nas_config', { id: id.value, config: config.value })
    message.value = 'Guardado solo para esta conexión. Reconecta el NAS para aplicar rutas y parámetros SSH.'
  } catch (e) { error.value = String(e) }
  finally { saving.value = false }
}
async function testScript() {
  if (!config.value || testing.value) return
  testing.value = true; test.value = null
  try { test.value = await invoke<StartupResult>('test_startup_script', { script: config.value.startup_script }) }
  catch (e) { error.value = String(e) }
  finally { testing.value = false }
}
</script>
<template>
  <section class="card nas-card">
    <div class="nas-heading"><div><h2 class="card-title">Configuración NAS por conexión</h2><p class="helper-text">Rutas, exclusiones y script previo independientes para cada NAS.</p></div></div>
    <div class="form-group"><label for="nas-connection">Conexión a configurar</label><select id="nas-connection" v-model="id" :disabled="saving || testing"><option :value="null">Selecciona una conexión</option><option v-for="c in connection.connections" :key="c.id!" :value="c.id">{{ c.name }} · {{ c.host }}</option></select></div>
    <div v-if="error" class="alert alert-error" role="alert">{{ error }}</div><div v-if="message" class="alert alert-success" role="status">{{ message }}</div>
    <div v-if="loading" role="status"><span class="loading-spinner" /> Cargando configuración…</div>
    <fieldset v-if="config" :disabled="saving || testing" class="nas-fields">
      <div class="nas-grid"><div v-for="[key, label] in fields" :key="key" class="form-group"><label :for="`nas-${key}`">{{ label }}</label><input :id="`nas-${key}`" v-model="config[key]" /></div>
      <div class="form-group"><label for="nas-timeout">Timeout SSH / comando (segundos)</label><input id="nas-timeout" v-model.number="config.ssh_timeout_secs" type="number" min="10" max="3600" /></div>
      <div class="form-group"><label for="nas-keepalive">Keepalive (segundos)</label><input id="nas-keepalive" v-model.number="config.keepalive_secs" type="number" min="5" max="120" /></div></div>
      <div class="form-group"><label for="nas-script">Script previo a esta conexión · cmd /C · máximo 120 segundos</label><textarea id="nas-script" v-model="config.startup_script" rows="3" placeholder="Comando para conectar la VPN de este NAS (opcional)" /></div>
      <div class="btn-group nas-actions"><button class="btn btn-primary" @click="save">Guardar cambios</button><button class="btn btn-secondary" :disabled="!config.startup_script.trim()" @click="testScript">Probar script</button></div>
    </fieldset>
    <p v-if="!connection.connections.length && !loading" class="empty-state">Crea primero una conexión NAS.</p>
    <pre v-if="test" class="alert" :class="test.success ? 'alert-success' : 'alert-error'">{{ test.output || (test.success ? 'Script completado' : 'El script falló') }}</pre>
  </section>
</template>
<style scoped>
.nas-card { box-shadow: none; }
.nas-heading { display: flex; justify-content: space-between; gap: 18px; margin-bottom: 10px; }
.nas-fields { border: 0; min-width: 0; }
.nas-grid { display: grid; grid-template-columns: repeat(2, minmax(0, 1fr)); gap: 0 18px; }
.nas-actions { margin-top: 6px; }
pre { white-space: pre-wrap; max-height: 240px; overflow: auto; margin-top: 16px; }
@media (max-width: 900px) { .nas-grid { grid-template-columns: 1fr; } }
</style>
