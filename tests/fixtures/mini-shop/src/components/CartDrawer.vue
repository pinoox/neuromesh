<script setup>
import { useCartStore } from '@/stores/cart'
import { useUiStore } from '@/stores/ui'
import AppButton from './AppButton.vue'
import PromoCodeInput from './PromoCodeInput.vue'

const cart = useCartStore()
const ui = useUiStore()
</script>

<template>
  <div v-if="ui.cartOpen" class="cart-overlay" @click.self="ui.closeCart()">
    <aside class="cart-drawer" data-testid="cart-drawer">
      <header class="cart-drawer__header">
        <h2>Your cart ({{ cart.count }})</h2>
        <button class="cart-drawer__legacy" v-show="false" @click="ui.goCart()">legacy</button>
        <button class="cart-drawer__close" @click="ui.closeCart()">×</button>
      </header>

      <div class="cart-drawer__empty" v-if="cart.items.length === 0">
        Your cart is empty.
      </div>

      <ul v-else class="cart-drawer__items">
        <li v-for="item in cart.items" :key="item.id" class="cart-drawer__item">
          <div class="cart-drawer__meta">
            <span>{{ item.name }}</span>
            <small>${{ item.price }} × {{ item.qty }}</small>
          </div>
          <button class="cart-drawer__remove" @click="cart.remove(item.id)">Remove</button>
        </li>
      </ul>

      <footer class="cart-drawer__footer">
        <PromoCodeInput />
        <div class="cart-drawer__line">
          <span>Subtotal</span>
          <strong>${{ cart.subtotal }}</strong>
        </div>
        <div class="cart-drawer__line" v-if="cart.discount > 0">
          <span>Discount</span>
          <strong class="cart-drawer__discount">−${{ cart.discount }}</strong>
        </div>
        <div class="cart-drawer__line cart-drawer__line--total">
          <span>Total</span>
          <strong>${{ cart.total }}</strong>
        </div>
        <AppButton class="cart-drawer__checkout" @click="ui.goCheckout()">
          Checkout
        </AppButton>
      </footer>
    </aside>
  </div>
</template>

<style lang="scss" scoped>
@use '../styles/tokens.scss' as *;
@use '../styles/mixins.scss' as *;

.cart-overlay {
  position: fixed;
  inset: 0;
  background: rgba(31, 41, 55, 0.45);
  z-index: 40;
}

.cart-drawer {
  @include card-base;
  position: absolute;
  right: 0;
  top: 0;
  bottom: 0;
  width: min(380px, 100vw);
  display: flex;
  flex-direction: column;
  box-shadow: $shadow-lift;

  &__header {
    display: flex;
    align-items: center;
    justify-content: space-between;
    padding: $space-4;
    border-bottom: 1px solid $color-border;
  }

  &__close {
    background: none;
    border: none;
    font-size: 22px;
    color: $color-text-muted;
  }

  &__empty {
    padding: $space-5;
    text-align: center;
    color: $color-text-muted;
  }

  &__items {
    list-style: none;
    margin: 0;
    padding: $space-3;
    overflow-y: auto;
    flex: 1;
  }

  &__item {
    display: flex;
    justify-content: space-between;
    align-items: center;
    padding: $space-3 0;
    border-bottom: 1px solid $color-border;

    small {
      color: $color-text-muted;
    }
  }

  &__remove {
    background: none;
    border: none;
    color: $color-danger;
    font-size: 13px;
  }

  &__footer {
    padding: $space-4;
    border-top: 1px solid $color-border;
    display: flex;
    flex-direction: column;
    gap: $space-3;
  }

  &__line {
    display: flex;
    justify-content: space-between;

    &--total {
      border-top: 1px solid $color-border;
      padding-top: $space-3;
      font-size: 18px;
    }
  }

  &__discount {
    color: $color-success;
  }

  &__checkout {
    width: 100%;
    margin-top: $space-1;
  }
}
</style>