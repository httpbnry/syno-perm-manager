<script setup lang="ts">
export interface PermissionInfo {
  principal: string
  origin: 'manual' | 'inherited' | 'group' | 'everyone'
  access: string
  permissions: string
  flags: string
  level: number
}
defineProps<{ entries: PermissionInfo[] }>()
const labels = { manual: 'Manual · usuario', inherited: 'Usuario · heredado', group: 'Por grupo', everyone: 'Todos' }
const rights: Record<string, string> = { r: 'Leer', w: 'Escribir', x: 'Atravesar / ejecutar', p: 'Añadir', d: 'Eliminar', D: 'Eliminar hijos', a: 'Leer atributos', A: 'Escribir atributos', R: 'Leer atributos extendidos', W: 'Escribir atributos extendidos', c: 'Leer ACL', C: 'Modificar ACL', o: 'Cambiar propietario', s: 'Sincronizar' }
function explain(value: string) { return [...value].filter(c => c !== '-').map(c => rights[c] || c).join(' · ') }
</script>
<template>
  <div v-for="(entry, index) in entries" :key="index" class="permission-entry">
    <div><span class="badge" :class="entry.origin === 'manual' ? 'badge-info' : 'badge-warning'">{{ labels[entry.origin] }}</span> <strong>{{ entry.principal || 'Todos' }}</strong></div>
    <div><span :class="entry.access === 'deny' ? 'diff-remove' : 'diff-add'">{{ entry.access === 'deny' ? 'Denegar' : 'Permitir' }}</span> <code>{{ entry.permissions }}</code></div>
    <small>{{ explain(entry.permissions) || 'Sin derechos marcados' }}</small>
    <small>Herencia: <code>{{ entry.flags }}</code> · {{ entry.level ? `Heredado, nivel ${entry.level}` : 'Definido en esta carpeta' }}</small>
  </div>
  <p v-if="!entries.length" class="helper-text">Sin entradas coincidentes en esta carpeta.</p>
</template>
<style scoped>
.permission-entry { border-left: 2px solid var(--border); padding: 8px 12px; margin-bottom: 10px; min-width: 210px; }
.permission-entry div { margin-bottom: 5px; }
.permission-entry small { display: block; color: var(--text-secondary); font-size: 11px; margin: 4px 0; }
.permission-entry strong { font-size: 12px; }
</style>
