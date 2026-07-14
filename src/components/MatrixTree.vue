<script setup lang="ts">
import type { TreeNode } from '../stores/explorer'

const props = defineProps<{
  nodes: TreeNode[]
  selectedPaths: Set<string>
  depth?: number
}>()

const emit = defineEmits<{
  togglePath: [path: string]
  toggleExpand: [node: TreeNode]
}>()

const depth = props.depth ?? 0
</script>

<template>
  <div class="matrix-tree">
    <div v-for="node in nodes" :key="node.path">
      <div
        class="tree-row"
        :style="{ paddingLeft: depth * 16 + 4 + 'px' }"
      >
        <span
          class="tree-toggle"
          @click.stop="emit('toggleExpand', node)"
        >
          <span v-if="node.loading" class="loading-spinner"></span>
          <span v-else-if="!node.loaded">&#9654;</span>
          <span v-else-if="node.expanded">&#9660;</span>
          <span v-else>&#9654;</span>
        </span>
        <input
          type="checkbox"
          class="tree-check"
          :checked="selectedPaths.has(node.path)"
          @change="emit('togglePath', node.path)"
        />
        <span class="tree-icon">&#128193;</span>
        <span class="tree-name" :title="node.path">{{ node.name }}</span>
      </div>
      <div v-if="node.expanded && node.children.length > 0">
        <MatrixTree
          :nodes="node.children"
          :selected-paths="selectedPaths"
          :depth="depth + 1"
          @toggle-path="emit('togglePath', $event)"
          @toggle-expand="emit('toggleExpand', $event)"
        />
      </div>
    </div>
  </div>
</template>

<style scoped>
.tree-row {
  display: flex;
  align-items: center;
  gap: 4px;
  padding: 2px 4px;
  border-radius: 3px;
  cursor: pointer;
}

.tree-row:hover {
  background: var(--bg-hover);
}

.tree-toggle {
  width: 14px;
  text-align: center;
  font-size: 8px;
  color: var(--text-muted);
  flex-shrink: 0;
}

.tree-check {
  width: 13px;
  height: 13px;
  accent-color: var(--accent);
  flex-shrink: 0;
}

.tree-icon {
  font-size: 11px;
  flex-shrink: 0;
}

.tree-name {
  font-size: 11px;
  font-family: monospace;
  white-space: nowrap;
  overflow: hidden;
  text-overflow: ellipsis;
}
</style>
