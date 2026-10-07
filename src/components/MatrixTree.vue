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
        <span class="tree-icon" aria-hidden="true"></span>
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
  gap: 7px;
  padding: 7px 8px;
  border-radius: 11px;
  cursor: pointer;
  color: var(--text-secondary);
  transition: background .12s ease, color .12s ease;
}

.tree-row:hover {
  background: var(--bg-hover);
  color: var(--text-primary);
}

.tree-toggle {
  width: 16px;
  text-align: center;
  font-size: 9px;
  color: var(--text-muted);
  flex-shrink: 0;
}

.tree-check {
  width: 14px;
  height: 14px;
  accent-color: var(--accent);
  flex-shrink: 0;
}

.tree-icon {
  width: 12px;
  height: 10px;
  border-radius: 3px;
  background: color-mix(in srgb, var(--accent) 42%, var(--bg-tertiary));
  flex-shrink: 0;
  position: relative;
}

.tree-icon::before {
  content: '';
  position: absolute;
  left: 1px;
  top: -3px;
  width: 7px;
  height: 4px;
  border-radius: 3px 3px 0 0;
  background: color-mix(in srgb, var(--accent) 34%, var(--bg-tertiary));
  flex-shrink: 0;
}

.tree-name {
  font-size: 12px;
  font-family: 'Cascadia Code', Consolas, monospace;
  white-space: nowrap;
  overflow: hidden;
  text-overflow: ellipsis;
}
</style>
