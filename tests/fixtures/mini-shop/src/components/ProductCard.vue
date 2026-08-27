<script setup>
import AppButton from './AppButton.vue'

defineProps({
  product: {
    type: Object,
    required: true,
  },
})

defineEmits(['view', 'add'])
</script>

<template>
  <article class="product-card" data-testid="product-card">
    <div class="product-card__media">
      <span class="product-card__badge" v-if="!product.inStock">Out of stock</span>
    </div>

    <div class="product-card__body">
      <span class="product-card__category">{{ product.category }}</span>
      <h3 class="product-card__title">{{ product.name }}</h3>
      <p class="product-card__description">{{ product.description }}</p>
    </div>

    <div class="product-card__footer">
      <strong class="product-card__price">${{ product.price }}</strong>
      <div class="product-card__actions">
        <AppButton variant="ghost" @click="$emit('view', product.id)">Details</AppButton>
        <AppButton :disabled="!product.inStock" @click="$emit('add', product)">
          Add
        </AppButton>
      </div>
    </div>
  </article>
</template>

<style lang="scss" scoped>
@use '../styles/tokens.scss' as *;
@use '../styles/mixins.scss' as *;

.product-card {
  @include card-base;
  @include hover-lift;
  overflow: hidden;
  display: flex;
  flex-direction: column;
  gap: $space-3;

  &:focus-within {
    outline: 2px solid $color-brand;
    outline-offset: 2px;
  }

  &__media {
    height: 140px;
    background:
      linear-gradient(135deg, lighten($color-brand, 28%), lighten($color-accent, 32%));
    position: relative;
    border-radius: $radius-lg $radius-lg 0 0;
  }

  &__badge {
    position: absolute;
    top: $space-2;
    left: $space-2;
    background: $color-danger;
    color: $color-surface;
    font-size: 12px;
    padding: $space-1 $space-2;
    border-radius: $radius-pill;
  }

  &__body {
    padding: 0 $space-3;
  }

  &__category {
    font-size: 11px;
    text-transform: uppercase;
    letter-spacing: 0.08em;
    color: $color-brand;
    font-weight: 600;
  }

  &__title {
    font-size: 18px;
    margin: $space-1 0;
  }

  &__description {
    color: $color-text-muted;
    font-size: 14px;
    margin: 0;
  }

  &__footer {
    margin-top: auto;
    padding: $space-3;
    display: flex;
    align-items: center;
    justify-content: space-between;
    border-top: 1px solid $color-border;
  }

  &__price {
    font-size: 18px;
    color: $color-brand-dark;
  }

  &__actions {
    display: flex;
    gap: $space-2;
  }
}
</style>