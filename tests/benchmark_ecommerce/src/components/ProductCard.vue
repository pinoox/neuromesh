<template>
  <div class="product-card" :class="{ 'is-featured': isFeatured }">
    <div class="product-card__image-wrapper">
      <img :src="product.image" :alt="product.title" class="product-card__image" loading="lazy" />
      <span v-if="product.badge" class="product-card__badge">{{ product.badge }}</span>
    </div>
    <div class="product-card__body">
      <span class="product-card__category">{{ product.category }}</span>
      <h3 class="product-card__title">{{ product.title }}</h3>
      <div class="product-card__price-row">
        <span class="product-card__price">{{ formatPrice(product.price) }}</span>
        <span v-if="product.originalPrice" class="product-card__original-price">{{ formatPrice(product.originalPrice) }}</span>
      </div>
      <button class="product-card__button" @click="handleAddToCart" :disabled="isAdding">
        {{ isAdding ? 'Adding...' : 'Add to Cart' }}
      </button>
    </div>
  </div>
</template>

<script setup lang="ts">
import { ref } from 'vue';
import type { Product } from '@/types/product';
import { useCartStore } from '@/stores/cartStore';
import { useCurrency } from '@/composables/useCurrency';

const props = defineProps<{
  product: Product;
  isFeatured?: boolean;
}>();

const cartStore = useCartStore();
const { formatPrice } = useCurrency();
const isAdding = ref(false);

async function handleAddToCart() {
  isAdding.value = true;
  await cartStore.addItem(props.product);
  setTimeout(() => {
    isAdding.value = false;
  }, 400);
}
</script>

<style lang="scss" scoped>
@use '@/styles/variables' as *;
@use '@/styles/breakpoints' as *;

.product-card {
  display: flex;
  flex-direction: column;
  background: $color-surface;
  border-radius: $radius-md;
  overflow: hidden;
  box-shadow: $shadow-sm;
  transition: transform 0.2s ease, box-shadow 0.2s ease;

  &:hover {
    transform: translateY(-4px);
    box-shadow: $shadow-md;
  }

  &__image-wrapper {
    position: relative;
    width: 100%;
    aspect-ratio: 1 / 1;
    background: $color-bg-alt;
  }

  &__image {
    width: 100%;
    height: 100%;
    object-fit: cover;
  }

  &__badge {
    position: absolute;
    top: $spacing-sm;
    left: $spacing-sm;
    background: $color-primary;
    color: white;
    padding: 2px $spacing-sm;
    border-radius: $radius-sm;
    font-size: $font-size-xs;
  }

  &__body {
    padding: $spacing-md;
    display: flex;
    flex-direction: column;
    flex: 1;
  }

  &__category {
    font-size: $font-size-xs;
    color: $color-text-muted;
    text-transform: uppercase;
  }

  &__title {
    font-size: $font-size-base;
    font-weight: 600;
    margin: $spacing-xs 0;
    color: $color-text-main;
  }

  &__price-row {
    display: flex;
    align-items: baseline;
    gap: $spacing-sm;
    margin-top: auto;
    margin-bottom: $spacing-md;
  }

  &__price {
    font-size: $font-size-lg;
    font-weight: 700;
    color: $color-primary;
  }

  &__original-price {
    font-size: $font-size-sm;
    color: $color-text-muted;
    text-decoration: line-through;
  }

  &__button {
    width: 100%;
    padding: $spacing-sm $spacing-md;
    background: $color-primary;
    color: white;
    border: none;
    border-radius: $radius-sm;
    font-weight: 600;
    cursor: pointer;
    transition: background 0.15s ease;

    &:hover:not(:disabled) {
      background: $color-primary-dark;
    }

    &:disabled {
      opacity: 0.6;
      cursor: not-allowed;
    }
  }

  @include respond-to('mobile') {
    &__body {
      padding: $spacing-sm;
    }
  }
}
</style>
