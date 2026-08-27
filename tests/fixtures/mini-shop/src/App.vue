<script setup>
import { useUiStore } from './stores/ui'
import Header from './components/Header.vue'
import CartDrawer from './components/CartDrawer.vue'
import CartView from './views/CartView.vue'
import CheckoutView from './views/CheckoutView.vue'

const ui = useUiStore()
</script>

<template>
  <Header />

  <CartView v-if="ui.currentView === 'cart'" />
  <CheckoutView v-else-if="ui.currentView === 'checkout'" />

  <CartDrawer />
</template>

<style lang="scss" scoped>
@use './styles/tokens.scss' as *;
@use './styles/mixins.scss' as *;
</style>
