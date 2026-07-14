<script setup lang="ts">
import type { TreeNode } from '../stores/explorer'
import type { PermColor } from '../types'

const props = defineProps<{
  nodes: TreeNode[]
  selectedPaths: Set<string>
  permColors?: Map<string, PermColor>
  overrides?: Set<string>
  onlyConflicts?: boolean
  depth?: number
}>()

const emit = defineEmits<{
  toggle: [node: TreeNode]
  select: [path: string]
}>()

const depth = props.depth ?? 0

function shouldShow(node: TreeNode): boolean {
  if (!props.onlyConflicts) return true
  if (props.overrides && props.overrides.has(node.path)) return true
  function hasOverrideInSubtree(n: TreeNode): boolean {
    if (props.overrides && props.overrides.has(n.path)) return true
    return n.children.some(hasOverrideInSubtree)
  }
  return hasOverrideInSubtree(node)
}
</script>

<template>
  <div class="tree-node">
    <template v-for="node in nodes" :key="node.path">
      <div v-if="shouldShow(node)">
        <div
          class="tree-node-row"
          :class="{ selected: selectedPaths.has(node.path) }"
          :style="{ paddingLeft: depth * 20 + 8 + 'px' }"
        >
          <span
            class="tree-toggle"
            @click.stop="emit('toggle', node)"
          >
            <span v-if="node.loading" class="loading-spinner"></span>
            <span v-else-if="!node.loaded">&#9654;</span>
            <span v-else-if="node.expanded">&#9660;</span>
            <span v-else>&#9654;</span>
          </span>
          <input
            type="checkbox"
            class="tree-checkbox"
            :checked="selectedPaths.has(node.path)"
            @change="emit('select', node.path)"
          />
          <span class="tree-icon">&#128193;</span>
          <span class="tree-label">{{ node.name }}</span>
          <span
            v-if="permColors && permColors.has(node.path)"
            class="perm-dot"
            :class="permColors.get(node.path)"
          ></span>
          <span
            v-if="overrides && overrides.has(node.path)"
            class="override-icon"
            title="Permiso distinto al de la carpeta padre"
          >&#9881;</span>
        </div>
        <div v-if="node.expanded && node.children.length > 0">
          <TreeView
            :nodes="node.children"
            :selected-paths="selectedPaths"
            :perm-colors="permColors"
            :overrides="overrides"
            :only-conflicts="onlyConflicts"
            :depth="depth + 1"
            @toggle="emit('toggle', $event)"
            @select="emit('select', $event)"
          />
        </div>
      </div>
    </template>
  </div>
</template>

<style scoped>
.perm-dot {
  display: inline-block;
  width: 10px;
  height: 10px;
  border-radius: 50%;
  margin-left: 6px;
  flex-shrink: 0;
}

.perm-dot.red { background: #e74c3c; }
.perm-dot.orange { background: #f39c12; }
.perm-dot.green { background: #2ecc71; }

.override-icon {
  margin-left: 4px;
  font-size: 11px;
  color: var(--warning);
  flex-shrink: 0;
}
</style>
