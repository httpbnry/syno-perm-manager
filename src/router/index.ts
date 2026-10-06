import { createRouter, createWebHistory } from 'vue-router'

const routes = [
  { path: '/', redirect: '/dashboard' },
  { path: '/dashboard', name: 'dashboard', component: () => import('../views/DashboardView.vue') },
  { path: '/connections', name: 'connections', component: () => import('../views/ConnectionsView.vue') },
  { path: '/settings', name: 'settings', component: () => import('../views/SettingsView.vue') },
  { path: '/explorer', name: 'explorer', component: () => import('../views/ExplorerView.vue') },
  { path: '/acl-editor', name: 'acl-editor', component: () => import('../views/AclEditorView.vue') },
  { path: '/matrix', name: 'matrix', component: () => import('../views/MatrixView.vue') },
  { path: '/compare', name: 'compare', component: () => import('../views/CompareView.vue') },
  { path: '/wizard', name: 'wizard', component: () => import('../views/WizardView.vue') },
  { path: '/users', name: 'users', component: () => import('../views/UsersView.vue') },
  { path: '/logs', name: 'logs', component: () => import('../views/LogsView.vue') },
]

export const router = createRouter({
  history: createWebHistory(),
  routes,
})
