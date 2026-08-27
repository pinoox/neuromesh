<script setup>
import { ref } from 'vue'
import { useCartStore } from '@/stores/cart'

const cart = useCartStore()
const input = ref('')

function submit() {
  if (!input.value.trim()) return
  cart.applyPromo(input.value)
  input.value = ''
}
</script>

<template>
  <form class="promo-code" @submit.prevent="submit">
    <input
      v-model="input"
      class="promo-code__input"
      placeholder="Promo code (try WELCOME10)"
      aria-label="Promo code"
    />
    <button class="promo-code__apply" type="submit">Apply</button>
  </form>
</template>

<style lang="scss" scoped>
@use '../styles/tokens.scss' as *;
@use '../styles/mixins.scss' as *;

.promo-code {
  display: flex;
  gap: $space-2;

  &__input {
    flex: 1;
    padding: $space-2;
    border: 1px solid $color-border;
    border-radius: $radius-sm;
    font: inherit;

    &:focus {
      outline: 2px solid $color-brand;
      outline-offset: -1px;
    }
  }

  &__apply {
    border: none;
    border-radius: $radius-sm;
    background: $color-accent;
    color: $color-text;
    font-weight: 700;
    padding: 0 $space-4;
  }
}
</style>