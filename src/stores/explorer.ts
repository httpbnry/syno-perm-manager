import { invoke } from '@tauri-apps/api/core'
import { defineStore } from 'pinia'
import { ref, watch } from 'vue'
import { cachedRead } from '../utils/readCache'
import { useConnectionStore } from './connection'
import type {
  ShareFolder, DirNode, AclEntry, AclDiff, ApplyResult, ApplyRequest,
  PathAcl, PermColor, SelectedPrincipal,
} from '../types'

export interface TreeNode {
  name: string
  path: string
  children: TreeNode[]
  expanded: boolean
  loading: boolean
  loaded: boolean
}

export const useExplorerStore = defineStore('explorer', () => {
  const connection = useConnectionStore()
  let epoch = 0
  let sharesRequest: Promise<void> | null = null
  let identityRequest: Promise<void> | null = null
  let sharesLoaded = 0
  const shares = ref<ShareFolder[]>([])
  const tree = ref<TreeNode[]>([])
  const selectedPaths = ref<Set<string>>(new Set())
  const users = ref<string[]>([])
  const groups = ref<string[]>([])
  const currentAcl = ref<AclEntry[]>([])
  const diffs = ref<AclDiff[]>([])
  const loading = ref(false)
  const applying = ref(false)
  const error = ref<string>('')

  const selectedPrincipal = ref<SelectedPrincipal | null>(null)
  const permColors = ref<Map<string, PermColor>>(new Map())
  const analyzing = ref(false)
  const analyzeProgress = ref({ current: 0, total: 0 })
  const overrides = ref<Set<string>>(new Set())
  const onlyConflicts = ref(false)

  watch(() => connection.sessionVersion, () => {
    epoch++; sharesRequest = null; identityRequest = null; sharesLoaded = 0
    tree.value = []; shares.value = []; users.value = []; groups.value = []
    clearSelection(); clearPermColors(); currentAcl.value = []; diffs.value = []
    loading.value = false; analyzing.value = false
  }, { flush: 'sync' })

  async function loadShares(force = false): Promise<void> {
    if (sharesRequest) return sharesRequest
    if (!force && sharesLoaded > Date.now() - 30_000) return
    const version = epoch
    const request = fetchShares(force, version)
    sharesRequest = request
    try { await request } finally { if (sharesRequest === request) sharesRequest = null }
  }
  async function fetchShares(force: boolean, version: number) {
    loading.value = true
    error.value = ''
    try {
      const data = await cachedRead<ShareFolder[]>('list_shares', undefined, force)
      if (version !== epoch) return
      shares.value = data
      sharesLoaded = Date.now()
      tree.value = shares.value.map((s) => ({
        name: s.name,
        path: s.path,
        children: [],
        expanded: false,
        loading: false,
        loaded: false,
      }))
    } catch (e: any) {
      error.value = String(e)
    } finally {
      if (version === epoch) loading.value = false
    }
  }

  async function expandNode(node: TreeNode) {
    if (node.loading) return
    const version = epoch
    if (node.loaded) {
      node.expanded = !node.expanded
      return
    }
    node.loading = true
    try {
      const dirs = await invoke<DirNode[]>('list_dirs', { path: node.path })
      if (version !== epoch) return
      node.children = dirs.map((d) => ({
        name: d.name,
        path: d.path,
        children: [],
        expanded: false,
        loading: false,
        loaded: false,
      }))
      node.loaded = true
      node.expanded = true
    } catch (e: any) {
      error.value = String(e)
    } finally {
      node.loading = false
    }
  }

  function toggleSelection(path: string) {
    if (selectedPaths.value.has(path)) {
      selectedPaths.value.delete(path)
    } else {
      selectedPaths.value.add(path)
    }
    selectedPaths.value = new Set(selectedPaths.value)
  }

  function selectAllChildren(node: TreeNode, selected: boolean) {
    if (selected) {
      selectedPaths.value.add(node.path)
    } else {
      selectedPaths.value.delete(node.path)
    }
    for (const child of node.children) {
      selectAllChildren(child, selected)
    }
    selectedPaths.value = new Set(selectedPaths.value)
  }

  function clearSelection() {
    selectedPaths.value = new Set()
  }

  async function loadUsersGroups(force = false): Promise<void> {
    if (identityRequest) return identityRequest
    const version = epoch
    const request = fetchUsersGroups(force, version)
    identityRequest = request
    try { await request } finally { if (identityRequest === request) identityRequest = null }
  }
  async function fetchUsersGroups(force: boolean, version: number) {
    error.value = ''
    try {
      const [u, g] = await Promise.all([
        cachedRead<string[]>('list_users', undefined, force),
        cachedRead<string[]>('list_groups', undefined, force),
      ])
      if (version !== epoch) return
      users.value = u
      groups.value = g
    } catch (e: any) {
      error.value = String(e)
    }
  }

  async function loadAcl(path: string) {
    const version = epoch
    error.value = ''
    try {
      const acl = await invoke<AclEntry[]>('get_acl', { path })
      if (version === epoch) currentAcl.value = acl
    } catch (e: any) {
      error.value = String(e)
    }
  }

  async function previewChanges(
    paths: string[],
    entries: AclEntry[],
    recursive: boolean
  ): Promise<AclDiff[]> {
    error.value = ''
    try {
      const request: ApplyRequest = { paths, entries, recursive }
      diffs.value = await invoke<AclDiff[]>('dry_run', { request })
      return diffs.value
    } catch (e: any) {
      error.value = String(e)
      return []
    }
  }

  async function applyChanges(
    paths: string[],
    entries: AclEntry[],
    recursive: boolean
  ): Promise<ApplyResult> {
    applying.value = true
    error.value = ''
    try {
      const request: ApplyRequest = { paths, entries, recursive }
      const result = await invoke<ApplyResult>('apply_acl', { request })
      clearPermColors()
      currentAcl.value = []
      diffs.value = []
      if (result.success) {
        clearSelection()
      }
      return result
    } catch (e: any) {
      error.value = String(e)
      return { success: false, paths_modified: 0, errors: [String(e)] }
    } finally {
      applying.value = false
    }
  }

  function collectVisiblePaths(nodes: TreeNode[]): string[] {
    const paths: string[] = []
    function walk(node: TreeNode) {
      paths.push(node.path)
      if (node.expanded) {
        for (const child of node.children) {
          walk(child)
        }
      }
    }
    for (const node of nodes) {
      walk(node)
    }
    return paths
  }

  function computeColor(
    entries: AclEntry[],
    principal: SelectedPrincipal
  ): PermColor {
    const ptype = principal.type
    const pname = principal.name

    let hasAllow = false
    let allowR = false
    let allowW = false
    let hasDeny = false

    for (const entry of entries) {
      const parts = entry.principal_type.split(':')
      const entryType = parts[0]
      const entryAllowDeny = parts[1] || 'allow'

      if (entryType !== ptype || entry.name !== pname) continue

      if (entryAllowDeny === 'deny') {
        hasDeny = true
      } else {
        hasAllow = true
        const perms = entry.permissions
        if (perms.includes('r')) allowR = true
        if (perms.includes('w')) allowW = true
      }
    }

    if (hasDeny && !hasAllow) return 'red'
    if (!hasAllow) return 'red'
    if (allowW) return 'green'
    if (allowR) return 'orange'
    return 'red'
  }

  function computeOverrides() {
    const result = new Set<string>()

    function walk(nodes: TreeNode[], parentColor: PermColor | null) {
      for (const node of nodes) {
        const myColor = permColors.value.get(node.path)
        if (myColor && parentColor && myColor !== parentColor) {
          result.add(node.path)
        }
        const nextParent = myColor || parentColor
        if (node.expanded && node.children.length > 0) {
          walk(node.children, nextParent)
        }
      }
    }

    walk(tree.value, null)
    overrides.value = result
  }

  async function analyzePerms() {
    if (analyzing.value) return
    const version = epoch
    if (!selectedPrincipal.value) return
    analyzing.value = true
    error.value = ''
    permColors.value = new Map()
    overrides.value = new Set()

    const visiblePaths = collectVisiblePaths(tree.value)
    analyzeProgress.value = { current: 0, total: visiblePaths.length }

    try {
      const results = await invoke<PathAcl[]>('analyze_perms', { paths: visiblePaths })
      if (version !== epoch) return
      const colors = new Map<string, PermColor>()
      for (const result of results) {
        const color = computeColor(result.entries, selectedPrincipal.value!)
        colors.set(result.path, color)
      }
      permColors.value = colors
      analyzeProgress.value = { current: visiblePaths.length, total: visiblePaths.length }
      computeOverrides()
      propagateSubfolderAccess()
    } catch (e: any) {
      if (version === epoch) error.value = String(e)
    } finally {
      if (version === epoch) analyzing.value = false
    }
  }

  function clearPermColors() {
    permColors.value = new Map()
    overrides.value = new Set()
    selectedPrincipal.value = null
    onlyConflicts.value = false
  }

  function propagateSubfolderAccess() {
    function hasAccessInChildren(node: TreeNode): boolean {
      for (const child of node.children) {
        const c = permColors.value.get(child.path)
        if (c === 'green' || c === 'orange') return true
        if (child.expanded && hasAccessInChildren(child)) return true
      }
      return false
    }

    function walk(nodes: TreeNode[]) {
      for (const node of nodes) {
        const myColor = permColors.value.get(node.path)
        if (myColor === 'red' && node.expanded && hasAccessInChildren(node)) {
          permColors.value.set(node.path, 'orange')
        }
        if (node.expanded) {
          walk(node.children)
        }
      }
    }

    walk(tree.value)
    permColors.value = new Map(permColors.value)
  }

  async function refreshCache() {
    try {
      await invoke('clear_acl_cache')
      if (selectedPrincipal.value) {
        await analyzePerms()
      }
    } catch (e: any) {
      error.value = String(e)
    }
  }

  return {
    shares,
    tree,
    selectedPaths,
    users,
    groups,
    currentAcl,
    diffs,
    loading,
    applying,
    error,
    selectedPrincipal,
    permColors,
    overrides,
    onlyConflicts,
    analyzing,
    analyzeProgress,
    loadShares,
    expandNode,
    toggleSelection,
    selectAllChildren,
    clearSelection,
    loadUsersGroups,
    loadAcl,
    previewChanges,
    applyChanges,
    analyzePerms,
    clearPermColors,
    refreshCache,
  }
})
