<script setup>
import { useCartStore } from '@/stores/cart'
import { useUiStore } from '@/stores/ui'
import AppButton from '@/components/AppButton.vue'
import PromoCodeInput from '@/components/PromoCodeInput.vue'

const cart = useCartStore()
const ui = useUiStore()

function placeOrder() {
  cart.clear()
  ui.showToast('Order placed — thank you!')
  ui.currentView = 'home'
}
</script>

<template>
  <main class="container" data-testid="cart-view">
    <h1>Your cart</h1>

    <p v-if="cart.items.length === 0" class="cart-view__empty">Nothing here yet.</p>

    <template v-else>
      <ul class="cart-view__list">
        <li v-for="item in cart.items" :key="item.id" class="cart-view__item">
          <div>
            <strong>{{ item.name }}</strong>
            <p>${{ item.price }} each</p>
          </div>
          <button class="cart-view__remove" @click="cart.remove(item.id)">Remove</button>
        </li>
      </ul>

      <aside class="cart-view__summary">
        <PromoCodeInput />
        <p class="cart-view__line">Subtotal <span>${{ cart.subtotal }}</span></p>
        <p v-if="cart.discount" class="cart-view__line cart-view__line--discount">
          Discount <span>−${{ cart.discount }}</span>
        </p>
        <p class="cart-view__line cart-view__line--total">
          Total <span>${{ cart.total }}</span>
        </p>
        <AppButton class="cart-view__order" @click="placeOrder">Place order</AppButton>
      </aside>
    </template>
  </main>
</template>

<style lang="scss" scoped>
@use '../styles/tokens.scss' as *;
@use '../styles/mixins.scss' as *;

.cart-view {
  &__list {
    list-style: none;
    margin: $space-4 0;
    padding: 0;
    display: grid;
    gap: $space-3;
  }

  &__item {
    @include card-base;
    display: flex;
    justify-content: space-between;
    align-items: center;
    padding: $space-4;

    p {
      margin: $space-1 0 0;
      color: $color-text-muted;
    }
  }

  &__remove {
    border: none;
    background: none;
    color: $color-danger;
  }

  &__summary {
    @include card-base;
    padding: $space-4;
    max-width: 360px;
  }

  &__line {
    display: flex;
    justify-content: space-between;
    margin: $space-2 0;

    &--discount span {
      color: $color-success;
    }

    &--total {
      font-size: 18px;
      font-weight: 700;
      border-top: 1px solid $color-border;
      padding-top: $space-3;
    }
  }

  &__order {
    width: 100%;
    margin-top: $space-3;
  }
}
</style>