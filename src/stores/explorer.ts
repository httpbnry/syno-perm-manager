import { invoke } from '@tauri-apps/api/core'
import { defineStore } from 'pinia'
import { ref } from 'vue'
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

  async function loadShares() {
    loading.value = true
    error.value = ''
    try {
      shares.value = await invoke<ShareFolder[]>('list_shares')
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
      loading.value = false
    }
  }

  async function expandNode(node: TreeNode) {
    if (node.loaded) {
      node.expanded = !node.expanded
      return
    }
    node.loading = true
    try {
      const dirs = await invoke<DirNode[]>('list_dirs', { path: node.path })
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

  async function loadUsersGroups() {
    error.value = ''
    try {
      const [u, g] = await Promise.all([
        invoke<string[]>('list_users'),
        invoke<string[]>('list_groups'),
      ])
      users.value = u
      groups.value = g
    } catch (e: any) {
      error.value = String(e)
    }
  }

  async function loadAcl(path: string) {
    error.value = ''
    try {
      currentAcl.value = await invoke<AclEntry[]>('get_acl', { path })
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
    if (!selectedPrincipal.value) return
    analyzing.value = true
    error.value = ''
    permColors.value = new Map()
    overrides.value = new Set()

    const visiblePaths = collectVisiblePaths(tree.value)
    analyzeProgress.value = { current: 0, total: visiblePaths.length }

    try {
      const results = await invoke<PathAcl[]>('analyze_perms', { paths: visiblePaths })
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
      error.value = String(e)
    }

    analyzing.value = false
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
