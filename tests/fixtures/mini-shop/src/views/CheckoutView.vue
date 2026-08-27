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
  <main class="container" data-testid="checkout-view">
    <h1>Checkout</h1>

    <p v-if="cart.items.length === 0" class="checkout__empty">Nothing to check out.</p>

    <template v-else>
      <section class="checkout__summary">
        <h2>Order summary</h2>
        <ul class="checkout__list">
          <li v-for="item in cart.items" :key="item.id" class="checkout__item">
            <div class="checkout__item-info">
              <span>{{ item.name }}</span>
              <div class="checkout__stepper">
                <button
                  type="button"
                  class="checkout__step"
                  @click="cart.setQty(item.id, item.qty - 1)"
                >−</button>
                <span class="checkout__step-value">{{ item.qty }}</span>
                <button
                  type="button"
                  class="checkout__step"
                  @click="cart.setQty(item.id, item.qty + 1)"
                >+</button>
              </div>
            </div>
            <strong>${{ item.price * item.qty }}</strong>
          </li>
        </ul>

        <PromoCodeInput />

        <p class="checkout__line">Subtotal <span>${{ cart.subtotal }}</span></p>
        <p v-if="cart.discount" class="checkout__line checkout__line--discount">
          Discount <span>−${{ cart.discount }}</span>
        </p>
        <p class="checkout__line checkout__line--total">
          Total <span>${{ cart.total }}</span>
        </p>

        <AppButton class="checkout__place" @click="placeOrder">Place order</AppButton>
        <AppButton variant="ghost" class="checkout__back" @click="ui.currentView = 'cart'">
          Back to cart
        </AppButton>
      </section>
    </template>
  </main>
</template>

<style lang="scss" scoped>
@use '../styles/tokens.scss' as *;
@use '../styles/mixins.scss' as *;

.checkout {
  &__empty {
    color: $color-text-muted;
  }

  &__summary {
    @include card-base;
    max-width: 440px;
    padding: $space-5;
  }

  &__list {
    list-style: none;
    margin: $space-3 0;
    padding: 0;
    display: grid;
    gap: $space-2;
    border-top: 1px solid $color-border;
  }

  &__item {
    display: flex;
    justify-content: space-between;
    align-items: center;
    padding-top: $space-3;
  }

  &__item-info {
    display: flex;
    flex-direction: column;
    gap: $space-2;
  }

  &__stepper {
    display: inline-flex;
    align-items: center;
    gap: $space-2;
  }

  &__step {
    width: 24px;
    height: 24px;
    border: 1px solid $color-border;
    border-radius: $radius-sm;
    background: $color-surface-alt;
    line-height: 1;

    &:hover {
      background: lighten($color-brand, 40%);
    }
  }

  &__step-value {
    min-width: 16px;
    text-align: center;
    font-weight: 600;
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

  &__place {
    width: 100%;
    margin-top: $space-4;
  }

  &__back {
    width: 100%;
    margin-top: $space-2;
  }
}
</style>