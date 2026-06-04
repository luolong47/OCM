import { createRouter, createWebHistory } from 'vue-router'

const routes = [
  { path: '/', redirect: '/providers' },
  {
    path: '/providers',
    name: 'providers',
    component: () => import('@/views/ProviderListView.vue'),
  },
  {
    path: '/providers/new',
    name: 'provider-new',
    component: () => import('@/views/ProviderEditView.vue'),
  },
  {
    path: '/providers/:id/edit',
    name: 'provider-edit',
    component: () => import('@/views/ProviderEditView.vue'),
    props: true,
  },
  {
    path: '/providers/:id/models',
    name: 'provider-models',
    component: () => import('@/views/ProviderModelManageView.vue'),
    props: true,
  },
  {
    path: '/settings',
    name: 'settings',
    component: () => import('@/views/SettingsView.vue'),
  },
]

export default createRouter({
  history: createWebHistory(),
  routes,
})
