<script setup>
defineProps({
  variant: {
    type: String,
    default: 'primary', // primary | ghost
  },
  disabled: {
    type: Boolean,
    default: false,
  },
})
</script>

<template>
  <button class="app-button" :class="`app-button--${variant}`" :disabled="disabled">
    <slot />
  </button>
</template>

<style lang="scss" scoped>
@use '../styles/tokens.scss' as *;
@use '../styles/mixins.scss' as *;

.app-button {
  border: none;
  border-radius: $radius-sm;
  padding: $space-2 $space-4;
  font-weight: 600;
  background: $color-brand;
  color: $color-surface;
  transition: background 0.15s ease, opacity 0.15s ease;

  &:hover:not(:disabled) {
    background: $color-brand-dark;
  }

  &:disabled {
    opacity: 0.5;
    cursor: not-allowed;
  }

  &--ghost {
    background: transparent;
    color: $color-brand;
    border: 1px solid $color-border;

    &:hover:not(:disabled) {
      background: $color-surface-alt;
    }
  }
}
</style>