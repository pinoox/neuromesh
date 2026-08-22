import { defineStore } from 'pinia';
import { ref, computed } from 'vue';
import type { Product, CartItem } from '@/types/product';

export const useCartStore = defineStore('cart', () => {
  const items = ref<CartItem[]>([]);
  const isOpen = ref(false);

  const totalCount = computed(() => {
    return items.value.reduce((total, item) => total + item.quantity, 0);
  });

  const totalPrice = computed(() => {
    return items.value.reduce((sum, item) => sum + item.product.price * item.quantity, 0);
  });

  function addItem(product: Product) {
    const existing = items.value.find(i => i.product.id === product.id);
    if (existing) {
      existing.quantity += 1;
    } else {
      items.value.push({ product, quantity: 1 });
    }
  }

  function removeItem(productId: string) {
    items.value = items.value.filter(i => i.product.id !== productId);
  }

  function toggleCart() {
    isOpen.value = !isOpen.value;
  }

  return {
    items,
    isOpen,
    totalCount,
    totalPrice,
    addItem,
    removeItem,
    toggleCart,
  };
});
