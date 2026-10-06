import { invoke } from '@tauri-apps/api/core'
import { defineStore } from 'pinia'
import { ref, computed } from 'vue'
import type { Connection, ConnectionInput } from '../types'
import { resetReadCache } from '../utils/readCache'

export const useConnectionStore = defineStore('connection', () => {
  const connections = ref<Connection[]>([])
  const connectedName = ref<string>('')
  const connectedId = ref<number | null>(null)
  const sessionVersion = ref(0)
  const connecting = ref(false)
  const isConnected = computed(() => connectedName.value !== '')
  const loading = ref(false)
  const error = ref<string>('')

  async function loadConnections() {
    loading.value = true
    error.value = ''
    try {
      connections.value = await invoke<Connection[]>('list_connections')
    } catch (e: any) {
      error.value = String(e)
    } finally {
      loading.value = false
    }
  }

  async function saveConnection(input: ConnectionInput): Promise<boolean> {
    error.value = ''
    try {
      await invoke('save_connection', { input })
      await loadConnections()
      return true
    } catch (e: any) {
      error.value = String(e)
      return false
    }
  }

  async function deleteConnection(id: number) {
    error.value = ''
    try {
      await invoke('delete_connection', { id })
      await loadConnections()
    } catch (e: any) {
      error.value = String(e)
    }
  }

  async function testConnection(
    host: string,
    port: number,
    username: string,
    password: string,
    useSudo: boolean,
    authMethod: string = 'password',
    keyPath: string | null = null,
    keyPassphrase: string | null = null,
  ): Promise<string> {
    const result = await invoke<string>('test_connection', {
      host,
      port,
      username,
      password,
      useSudo,
      authMethod,
      keyPath,
      keyPassphrase,
    })
    return result
  }

  async function connect(id: number): Promise<boolean> {
    if (connecting.value) return false
    connecting.value = true
    error.value = ''
    try {
      connectedName.value = await invoke<string>('connect_to_nas', { id })
      connectedId.value = id
      resetReadCache(); sessionVersion.value++
      return true
    } catch (e: any) {
      error.value = String(e)
      return false
    } finally {
      connecting.value = false
    }
  }

  async function disconnect() {
    try {
      await invoke('disconnect')
      connectedName.value = ''
      connectedId.value = null
      resetReadCache(); sessionVersion.value++
    } catch (e: any) {
      error.value = String(e)
    }
  }

  return {
    connections,
    connectedName,
    connectedId,
    sessionVersion,
    connecting,
    isConnected,
    loading,
    error,
    loadConnections,
    saveConnection,
    deleteConnection,
    testConnection,
    connect,
    disconnect,
  }
})
