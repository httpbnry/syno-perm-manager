<script setup lang="ts">
import { onMounted, ref } from 'vue'
import { useRoute } from 'vue-router'
import { invoke } from '@tauri-apps/api/core'
import { getVersion } from '@tauri-apps/api/app'
import { openUrl } from '@tauri-apps/plugin-opener'
import NasSettings from '../components/NasSettings.vue'
import type { AppConfig } from '../types'
const route = useRoute()
const section = ref(route.query.connection ? 'nas' : 'general')
const config = ref<AppConfig | null>(null)
const stats = ref({ logs: 0, snapshots: 0 })
const error = ref('')
const message = ref('')
const busy = ref(false)
const version = ref('1.0.0')
const sections = [{ id: 'general', label: 'General' }, { id: 'nas', label: 'Conexiones NAS' }, { id: 'data', label: 'Datos' }, { id: 'about', label: 'Acerca de / About' }]
onMounted(async () => {
  try {
    const [c, s, v] = await Promise.all([invoke<AppConfig>('get_config'), invoke<typeof stats.value>('get_db_stats'), getVersion()])
    config.value = c; stats.value = s; version.value = v
  } catch (e) { error.value = String(e) }
})
async function save() {
  if (!config.value || busy.value) return
  busy.value = true; error.value = ''; message.value = ''
  try { await invoke('save_config', { dto: config.value }); document.documentElement.setAttribute('data-theme', config.value.theme); message.value = 'Preferencias guardadas.' }
  catch (e) { error.value = String(e) } finally { busy.value = false }
}
async function clear(command: string, label: string) {
  if (!confirm(`¿${label}? Esta acción no se puede deshacer.`)) return
  try { await invoke(command); stats.value = await invoke('get_db_stats'); message.value = 'Operación completada.' }
  catch (e) { error.value = String(e) }
}
async function exportConfig() {
  try {
    const data = await invoke<string>('export_config')
    const url = URL.createObjectURL(new Blob([data], { type: 'application/json' }))
    const link = document.createElement('a'); link.href = url; link.download = 'syno-perm-config.json'; link.click(); URL.revokeObjectURL(url)
  } catch (e) { error.value = String(e) }
}
async function importConfig(event: Event) {
  const input = event.target as HTMLInputElement
  const file = input.files?.[0]
  if (!file) return
  try { await invoke('import_config', { json: await file.text() }); message.value = 'Configuración importada. Se han omitido los scripts de inicio. Reinicia la aplicación.' }
  catch (e) { error.value = String(e) } finally { input.value = '' }
}
async function github() { try { await openUrl('https://github.com/httpbnry') } catch (e) { error.value = String(e) } }
</script>
<template>
  <div class="view-header"><div class="eyebrow">PREFERENCIAS</div><h1>Configuración</h1><p>Personaliza la aplicación y configura cada NAS de forma independiente.</p></div>
  <div v-if="error" class="alert alert-error" role="alert">{{ error }}</div><div v-if="message" class="alert alert-success" role="status">{{ message }}</div>
  <div class="card btn-group"><button v-for="s in sections" :key="s.id" class="btn" :class="section === s.id ? 'btn-primary' : 'btn-secondary'" @click="section = s.id">{{ s.label }}</button></div>
  <NasSettings v-if="section === 'nas'" :initial-id="Number(route.query.connection) || undefined" />
  <section v-if="section === 'general' && config" class="card">
    <h2 class="card-title">Preferencias generales</h2>
    <div class="form-group"><label for="theme">Apariencia</label><select id="theme" v-model="config.theme"><option value="dark">Oscuro</option><option value="light">Claro</option></select></div>
    <p class="helper-text">Los ajustes del NAS, incluido el script de VPN y los parámetros SSH, se encuentran en «Conexiones NAS». La interfaz está disponible en español.</p>
    <button class="btn btn-primary" :disabled="busy" @click="save">Guardar preferencias</button>
  </section>
  <section v-if="section === 'data'" class="card">
    <h2 class="card-title">Datos locales y copias</h2>
    <p class="helper-text">{{ stats.logs }} registros de auditoría · {{ stats.snapshots }} snapshots ACL. Los respaldos de comparación se exportan desde el historial.</p>
    <div class="btn-group"><button class="btn btn-secondary" @click="clear('clear_acl_cache', 'Limpiar la caché ACL de la conexión activa')">Limpiar caché ACL</button><button class="btn btn-danger" @click="clear('clear_logs', 'Borrar todo el historial, incluidos los respaldos de comparación')">Borrar historial</button><button class="btn btn-danger" @click="clear('clear_snapshots', 'Borrar todos los snapshots ACL')">Borrar snapshots</button></div>
    <h3 class="card-title" style="margin-top: 28px">Configuración portátil</h3><p class="helper-text">Exporta conexiones, perfiles NAS y preferencias. Las credenciales no se exportan y los scripts se omiten al importar.</p>
    <div class="btn-group"><button class="btn btn-secondary" @click="exportConfig">Exportar configuración</button><label class="btn btn-secondary">Importar JSON<input type="file" accept=".json" class="file-input" @change="importConfig" /></label></div>
  </section>
  <section v-if="section === 'about'" class="card about">
    <span class="brand-mark">S</span><div class="eyebrow">SYNOLOGY · USUARIOS · PERMISOS</div><h2>Syno Perm Manager</h2><p class="helper-text">Versión {{ version }}</p>
    <p>Aplicación de escritorio para administrar usuarios, grupos y permisos ACL de servidores Synology mediante SSH. Nació para simplificar las altas de usuarios y tareas como «que tenga los mismos permisos que otra persona», evitando repetir ajustes carpeta por carpeta.</p>
    <ul><li>Comparación de grupos y permisos manuales de carpetas.</li><li>Explorador, editor ACL y matriz de permisos.</li><li>Perfiles independientes para cada conexión NAS.</li><li>Historial de cambios, respaldos y exportación de informes.</li></ul>
    <dl><dt>Creación del proyecto</dt><dd>14 de julio de 2026 · fecha del primer commit del repositorio.</dd><dt>Autor / GitHub</dt><dd><button class="btn btn-secondary" @click="github">httpbnry · github.com/httpbnry ↗</button></dd><dt>Tecnología</dt><dd>Vue 3 · TypeScript · Tauri 2 · Rust · SQLite</dd><dt>Licencia</dt><dd>CC BY-NC-SA 4.0 · httpbnry</dd></dl>
  </section>
</template>
<style scoped>
.file-input { max-width: 180px; font-size: 11px; }
.about { max-width: 900px; }
.about .brand-mark { margin-bottom: 18px; }
.about h2 { font-size: 28px; margin: 8px 0; }
.about p, .about li, .about dd { color: var(--text-secondary); line-height: 1.8; }
.about ul { margin: 20px; }
.about dt { font-weight: 600; margin-top: 18px; }
</style>
