<script setup>
import { useCartStore } from '@/stores/cart'
import { useUiStore } from '@/stores/ui'

const cart = useCartStore()
const ui = useUiStore()
</script>

<template>
  <header class="header">
    <div class="container header__inner">
      <button class="header__logo" @click="ui.goCart(); ui.currentView = 'home'">
        ☁ Nimbus
      </button>

      <nav class="header__nav">
        <button @click="ui.currentView = 'home'">Shop</button>
        <button class="header__cart" @click="ui.openCart()" aria-label="Open cart">
          Cart
          <span v-if="cart.count > 0" class="header__count">{{ cart.count }}</span>
        </button>
      </nav>
    </div>
  </header>
</template>

<style lang="scss" scoped>
@use '../styles/tokens.scss' as *;
@use '../styles/mixins.scss' as *;

.header {
  background: $color-surface;
  border-bottom: 1px solid $color-border;
  position: sticky;
  top: 0;
  z-index: 30;

  &__inner {
    display: flex;
    align-items: center;
    justify-content: space-between;
    padding-top: $space-3;
    padding-bottom: $space-3;
  }

  &__logo {
    background: none;
    border: none;
    font-size: 20px;
    font-weight: 700;
    color: $color-brand;
  }

  &__nav {
    display: flex;
    gap: $space-4;
    align-items: center;

    button {
      background: none;
      border: none;
      color: $color-text;
    }
  }

  &__cart {
    position: relative;
  }

  &__count {
    position: absolute;
    top: -8px;
    right: -10px;
    background: $color-accent;
    color: $color-text;
    font-size: 11px;
    font-weight: 700;
    width: 18px;
    height: 18px;
    border-radius: $radius-pill;
    display: grid;
    place-items: center;
  }
}
</style>