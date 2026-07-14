<script setup lang="ts">
import { ref, onMounted, watch } from 'vue'
import { invoke } from '@tauri-apps/api/core'
import type { StartupResult, AppConfig } from '../types'

const activeSection = ref<'general' | 'nas' | 'startup' | 'security' | 'data'>('general')

const startupScript = ref('')
const testingScript = ref(false)
const testResult = ref<StartupResult | null>(null)
const scriptSaved = ref(false)

const config = ref<AppConfig>({
  volume_path: '/volume1',
  synoacltool_path: '/usr/syno/bin/synoacltool',
  synoshare_path: '/usr/syno/sbin/synoshare',
  synouser_path: '/usr/syno/sbin/synouser',
  synogroup_path: '/usr/syno/sbin/synogroup',
  find_path: '/bin/find',
  excluded_folders: '@eaDir, #recycle',
  ssh_timeout_secs: 600,
  keepalive_secs: 15,
  log_retention_days: 90,
  snapshot_retention_count: 100,
  password_length: 12,
  password_special_chars: true,
  username_format: '{first_initial}{last_name}',
  description_template: '{full_name} Alta {date} {password}',
  theme: 'dark',
  language: 'es',
})

const configSaved = ref(false)
const configLoading = ref(false)

const dbStats = ref({ logs: 0, snapshots: 0 })

onMounted(async () => {
  try {
    const c = await invoke<AppConfig>('get_config')
    config.value = c
  } catch {}
  try {
    const logs = await invoke<any[]>('list_logs', { limit: 999999 })
    dbStats.value.logs = logs.length
  } catch {}
  try {
    const snaps = await invoke<any[]>('list_snapshots', { limit: 999999 })
    dbStats.value.snapshots = snaps.length
  } catch {}
  try {
    const script = await invoke<string | null>('get_app_setting', { key: 'startup_script' })
    if (script) startupScript.value = script
  } catch {}
})

async function saveConfig() {
  configLoading.value = true
  try {
    await invoke('save_config', { dto: config.value })
    document.documentElement.setAttribute('data-theme', config.value.theme)
    configSaved.value = true
    setTimeout(() => { configSaved.value = false }, 2000)
  } catch (e: any) {
    alert(String(e))
  } finally {
    configLoading.value = false
  }
}

async function saveStartupScript() {
  try {
    await invoke('set_app_setting', { key: 'startup_script', value: startupScript.value })
    scriptSaved.value = true
    setTimeout(() => { scriptSaved.value = false }, 2000)
  } catch (e: any) { alert(String(e)) }
}

async function testStartupScript() {
  if (!startupScript.value.trim()) return
  testingScript.value = true
  testResult.value = null
  try {
    testResult.value = await invoke<StartupResult>('test_startup_script', { script: startupScript.value })
  } catch (e: any) {
    testResult.value = { ran: true, output: String(e), success: false }
  } finally { testingScript.value = false }
}

async function clearStartupScript() {
  startupScript.value = ''
  testResult.value = null
  try { await invoke('set_app_setting', { key: 'startup_script', value: '' }) } catch {}
}

async function clearLogs() {
  if (!confirm('Borrar todos los logs?')) return
  try { await invoke('clear_logs'); dbStats.value.logs = 0 } catch (e: any) { alert(String(e)) }
}

async function exportConfig() {
  try {
    const json = await invoke<string>('export_config')
    const blob = new Blob([json], { type: 'application/json' })
    const url = URL.createObjectURL(blob)
    const a = document.createElement('a')
    a.href = url
    a.download = `syno-perm-config-${new Date().toISOString().slice(0, 10)}.json`
    a.click()
    URL.revokeObjectURL(url)
  } catch (e: any) { alert(String(e)) }
}

async function importConfig() {
  const input = document.createElement('input')
  input.type = 'file'
  input.accept = '.json'
  input.onchange = async () => {
    const file = input.files?.[0]
    if (!file) return
    const text = await file.text()
    try {
      await invoke('import_config', { json: text })
      alert('Configuracion importada. Reinicia la app para aplicar.')
    } catch (e: any) { alert(String(e)) }
  }
  input.click()
}

async function clearSnapshots() {
  if (!confirm('Borrar todos los snapshots?')) return
  try { await invoke('clear_snapshots'); dbStats.value.snapshots = 0 } catch (e: any) { alert(String(e)) }
}

async function clearCache() {
  try { await invoke('clear_acl_cache') } catch (e: any) { alert(String(e)) }
}

watch(() => config.value.theme, (newTheme) => {
  document.documentElement.setAttribute('data-theme', newTheme)
})
</script>

<template>
  <div class="view-header">
    <h1>Configuracion</h1>
    <p>Ajustes de la aplicacion y del NAS</p>
  </div>

  <div class="card" style="padding: 0 14px 10px;">
    <div style="display: flex; gap: 4px; padding: 10px 0; flex-wrap: wrap;">
      <button class="btn btn-sm" :class="activeSection === 'general' ? 'btn-primary' : 'btn-secondary'" @click="activeSection = 'general'">General</button>
      <button class="btn btn-sm" :class="activeSection === 'nas' ? 'btn-primary' : 'btn-secondary'" @click="activeSection = 'nas'">NAS</button>
      <button class="btn btn-sm" :class="activeSection === 'startup' ? 'btn-primary' : 'btn-secondary'" @click="activeSection = 'startup'">Inicio</button>
      <button class="btn btn-sm" :class="activeSection === 'security' ? 'btn-primary' : 'btn-secondary'" @click="activeSection = 'security'">Seguridad</button>
      <button class="btn btn-sm" :class="activeSection === 'data' ? 'btn-primary' : 'btn-secondary'" @click="activeSection = 'data'">Datos</button>
    </div>
  </div>

  <!-- GENERAL -->
  <div v-if="activeSection === 'general'" class="card">
    <div class="card-title">Preferencias generales</div>

    <div class="setting-row">
      <div>
        <div class="setting-label">Longitud de password generada</div>
        <div class="setting-desc">Numero de caracteres de las passwords aleatorias</div>
      </div>
      <input v-model.number="config.password_length" type="number" min="6" max="32" class="input-mini" style="width: 80px;" @change="saveConfig" />
    </div>

    <div class="setting-row">
      <div>
        <div class="setting-label">Caracteres especiales en password</div>
        <div class="setting-desc">Incluye !@#% en las passwords generadas</div>
      </div>
      <label class="toggle">
        <input type="checkbox" v-model="config.password_special_chars" @change="saveConfig" />
        <span class="toggle-slider"></span>
      </label>
    </div>

    <div class="setting-row">
      <div>
        <div class="setting-label">Formato de username</div>
        <div class="setting-desc">Como se genera el usuario desde el nombre completo</div>
      </div>
      <input v-model="config.username_format" class="input-mini" style="width: 240px;" @change="saveConfig" />
    </div>

    <div class="setting-row">
      <div>
        <div class="setting-label">Plantilla de descripcion</div>
        <div class="setting-desc">Variables: {full_name} {date} {password}</div>
      </div>
      <input v-model="config.description_template" class="input-mini" style="width: 280px;" @change="saveConfig" />
    </div>

    <div class="setting-row">
      <div>
        <div class="setting-label">Tema</div>
        <div class="setting-desc">Apariencia de la aplicacion</div>
      </div>
      <select v-model="config.theme" class="select-mini" @change="saveConfig">
        <option value="dark">Oscuro</option>
        <option value="light">Claro</option>
      </select>
    </div>

    <div class="setting-row">
      <div>
        <div class="setting-label">Idioma</div>
        <div class="setting-desc">Idioma de la interfaz</div>
      </div>
      <select v-model="config.language" class="select-mini" @change="saveConfig">
        <option value="es">Espanol</option>
        <option value="en">English</option>
      </select>
    </div>
  </div>

  <!-- NAS -->
  <div v-if="activeSection === 'nas'" class="card">
    <div class="card-title">Configuracion del NAS</div>
    <p style="font-size: 12px; color: var(--text-muted); margin-bottom: 14px;">
      Ajusta las rutas si tu Synology tiene una estructura distinta o usa volumes diferentes.
    </p>

    <div class="setting-row">
      <div>
        <div class="setting-label">Volume base</div>
        <div class="setting-desc">Ruta del volume principal (ej: /volume1, /volume2, /volumeUSB1)</div>
      </div>
      <input v-model="config.volume_path" class="input-mini" style="width: 200px;" @change="saveConfig" />
    </div>

    <div class="setting-row">
      <div>
        <div class="setting-label">Ruta synoacltool</div>
        <div class="setting-desc">Binario de gestion de ACLs</div>
      </div>
      <input v-model="config.synoacltool_path" class="input-mini" style="width: 240px;" @change="saveConfig" />
    </div>

    <div class="setting-row">
      <div>
        <div class="setting-label">Ruta synoshare</div>
        <div class="setting-desc">Binario de carpetas compartidas</div>
      </div>
      <input v-model="config.synoshare_path" class="input-mini" style="width: 240px;" @change="saveConfig" />
    </div>

    <div class="setting-row">
      <div>
        <div class="setting-label">Ruta synouser</div>
        <div class="setting-desc">Binario de usuarios</div>
      </div>
      <input v-model="config.synouser_path" class="input-mini" style="width: 240px;" @change="saveConfig" />
    </div>

    <div class="setting-row">
      <div>
        <div class="setting-label">Ruta synogroup</div>
        <div class="setting-desc">Binario de grupos</div>
      </div>
      <input v-model="config.synogroup_path" class="input-mini" style="width: 240px;" @change="saveConfig" />
    </div>

    <div class="setting-row">
      <div>
        <div class="setting-label">Ruta find</div>
        <div class="setting-desc">Binario para listar carpetas</div>
      </div>
      <input v-model="config.find_path" class="input-mini" style="width: 200px;" @change="saveConfig" />
    </div>

    <div class="setting-row">
      <div>
        <div class="setting-label">Carpetas excluidas</div>
        <div class="setting-desc">Separadas por coma. Estas carpetas no aparecen en el arbol</div>
      </div>
      <input v-model="config.excluded_folders" class="input-mini" style="width: 240px;" @change="saveConfig" />
    </div>

    <div class="btn-group" style="margin-top: 14px;">
      <button class="btn btn-primary" @click="saveConfig" :disabled="configLoading">
        <span v-if="configSaved" style="color: var(--success);">&#10003; Guardado</span>
        <span v-else>Guardar configuracion</span>
      </button>
    </div>
  </div>

  <!-- INICIO -->
  <div v-if="activeSection === 'startup'" class="card">
    <div class="card-title">Script de inicio (pre-conexion)</div>
    <p style="font-size: 12px; color: var(--text-muted); margin-bottom: 12px;">
      Se ejecuta antes de conectar al NAS. Util para iniciar VPN, comprobar conectividad, etc.
    </p>

    <div class="form-group">
      <label>Comando (se ejecuta con cmd /C)</label>
      <textarea v-model="startupScript" rows="4" class="script-textarea"
        placeholder="C:\PROGRA~2\Sophos\Connect\sccli.exe enable -n tu_conexion_vpn"></textarea>
    </div>

    <div class="alert alert-info" style="margin-bottom: 12px; font-size: 12px;">
      <strong>Ejemplos:</strong><br>
      <code>C:\PROGRA~2\Sophos\Connect\sccli.exe enable -n tu_conexion_vpn</code> - Sophos VPN<br>
      <code>ping -n 1 192.168.1.100 &gt;nul 2&gt;&amp;1 || echo NAS no alcanzable</code> - Comprobar<br>
      <code>timeout /t 10 /nobreak</code> - Esperar 10s<br>
      <code>net use Z: \\192.168.1.100\share</code> - Montar unidad
    </div>

    <div class="btn-group">
      <button class="btn btn-primary" @click="saveStartupScript">
        <span v-if="scriptSaved" style="color: var(--success);">&#10003;</span>
        Guardar
      </button>
      <button class="btn btn-secondary" @click="testStartupScript" :disabled="testingScript || !startupScript.trim()">
        <span v-if="testingScript" class="loading-spinner"></span>
        Probar
      </button>
      <button class="btn btn-danger btn-sm" @click="clearStartupScript" :disabled="!startupScript.trim()">Borrar</button>
    </div>

    <div v-if="testResult" style="margin-top: 12px;">
      <div :class="testResult.success ? 'alert alert-success' : 'alert alert-error'" style="font-size: 12px;">
        <strong>{{ testResult.success ? 'Ejecutado correctamente' : 'Error' }}</strong>
        <pre style="margin-top: 6px; white-space: pre-wrap; font-size: 11px; max-height: 200px; overflow-y: auto;">{{ testResult.output }}</pre>
      </div>
    </div>
  </div>

  <!-- SEGURIDAD -->
  <div v-if="activeSection === 'security'" class="card">
    <div class="card-title">Seguridad</div>

    <div class="setting-row">
      <div>
        <div class="setting-label">SSH timeout (segundos)</div>
        <div class="setting-desc">Tiempo maximo de inactividad antes de cerrar la conexion</div>
      </div>
      <input v-model.number="config.ssh_timeout_secs" type="number" min="60" max="3600" class="input-mini" style="width: 80px;" @change="saveConfig" />
    </div>

    <div class="setting-row">
      <div>
        <div class="setting-label">Keepalive (segundos)</div>
        <div class="setting-desc">Intervalo de paquetes keepalive para mantener la conexion</div>
      </div>
      <input v-model.number="config.keepalive_secs" type="number" min="5" max="120" class="input-mini" style="width: 80px;" @change="saveConfig" />
    </div>

    <div class="alert alert-warning" style="margin-top: 16px; font-size: 12px;">
      <strong>Estado de seguridad:</strong>
      <ul style="margin: 8px 0 0 16px;">
        <li>Credenciales en Windows Credential Manager (no texto plano)</li>
        <li>Host key verification en SQLite (proteccion MITM)</li>
        <li>Shell escaping en todos los comandos SSH</li>
        <li>Password via stdin del canal SSH (no visible en ps)</li>
        <li>Soporte de clave SSH restringida (defense-in-depth)</li>
        <li>SQLite en modo WAL</li>
      </ul>
    </div>
  </div>

  <!-- DATOS -->
  <div v-if="activeSection === 'data'" class="card">
    <div class="card-title">Gestion de datos</div>

    <div class="setting-row">
      <div>
        <div class="setting-label">Retencion de logs (dias)</div>
        <div class="setting-desc">Logs mas antiguos se borran automaticamente</div>
      </div>
      <input v-model.number="config.log_retention_days" type="number" min="1" max="365" class="input-mini" style="width: 80px;" @change="saveConfig" />
    </div>

    <div class="setting-row">
      <div>
        <div class="setting-label">Snapshots a mantener</div>
        <div class="setting-desc">Numero maximo de snapshots de ACL guardados</div>
      </div>
      <input v-model.number="config.snapshot_retention_count" type="number" min="10" max="1000" class="input-mini" style="width: 80px;" @change="saveConfig" />
    </div>

    <div style="border-top: 1px solid var(--border); margin: 14px 0;"></div>

    <div class="setting-row">
      <div>
        <div class="setting-label">Cache de ACLs</div>
        <div class="setting-desc">Acelera el analisis de permisos</div>
      </div>
      <button class="btn btn-danger btn-sm" @click="clearCache">Limpiar cache</button>
    </div>

    <div class="setting-row">
      <div>
        <div class="setting-label">Logs de auditoria ({{ dbStats.logs }})</div>
        <div class="setting-desc">Historial de cambios</div>
      </div>
      <button class="btn btn-danger btn-sm" @click="clearLogs">Borrar logs</button>
    </div>

    <div class="setting-row">
      <div>
        <div class="setting-label">Snapshots de ACL ({{ dbStats.snapshots }})</div>
        <div class="setting-desc">Backups de permisos para deshacer</div>
      </div>
      <button class="btn btn-danger btn-sm" @click="clearSnapshots">Borrar snapshots</button>
    </div>

    <div style="border-top: 1px solid var(--border); margin: 14px 0;"></div>

    <div class="setting-row">
      <div>
        <div class="setting-label">Exportar configuracion</div>
        <div class="setting-desc">Descarga conexiones, ajustes y script de inicio en JSON</div>
      </div>
      <button class="btn btn-secondary btn-sm" @click="exportConfig">Exportar</button>
    </div>

    <div class="setting-row">
      <div>
        <div class="setting-label">Importar configuracion</div>
        <div class="setting-desc">Restaura desde un archivo JSON exportado</div>
      </div>
      <button class="btn btn-secondary btn-sm" @click="importConfig">Importar</button>
    </div>

    <div class="alert alert-warning" style="margin-top: 14px; font-size: 12px;">
      BD en <code>%APPDATA%\syno-perm-manager\syno-perm-manager.db</code>
    </div>
  </div>
</template>

<style scoped>
.setting-row {
  display: flex;
  align-items: center;
  justify-content: space-between;
  padding: 14px 0;
  border-bottom: 1px solid var(--border);
  gap: 16px;
}
.setting-row:last-child { border-bottom: none; }
.setting-label { font-size: 13px; font-weight: 500; }
.setting-desc { font-size: 11px; color: var(--text-muted); margin-top: 2px; }

.toggle { position: relative; display: inline-block; width: 40px; height: 22px; flex-shrink: 0; }
.toggle input { opacity: 0; width: 0; height: 0; }
.toggle-slider {
  position: absolute; cursor: pointer; top: 0; left: 0; right: 0; bottom: 0;
  background: var(--bg-input); border: 1px solid var(--border); border-radius: 22px; transition: 0.2s;
}
.toggle-slider:before {
  content: ""; position: absolute; height: 16px; width: 16px; left: 2px; bottom: 2px;
  background: var(--text-muted); border-radius: 50%; transition: 0.2s;
}
.toggle input:checked + .toggle-slider { background: var(--accent); border-color: var(--accent); }
.toggle input:checked + .toggle-slider:before { transform: translateX(18px); background: white; }

.select-mini, .input-mini {
  padding: 5px 8px; font-size: 12px; background: var(--bg-input);
  border: 1px solid var(--border); border-radius: 5px; color: var(--text-primary); outline: none;
}
.script-textarea {
  width: 100%; padding: 8px 12px; font-size: 12px; font-family: 'Consolas', monospace;
  background: var(--bg-input); border: 1px solid var(--border); border-radius: 6px;
  color: var(--text-primary); outline: none; resize: vertical;
}
.alert-info {
  background: rgba(78, 154, 241, 0.15); border: 1px solid rgba(78, 154, 241, 0.3);
  color: var(--accent); padding: 10px 14px; border-radius: var(--radius); font-size: 13px;
}
.alert-info code { font-family: monospace; font-size: 11px; color: var(--text-primary); }
</style>
