import { defineStore } from 'pinia'
import { products } from '../data/products'

const PROMO_CODES = {
  WELCOME10: 0.1,
  NIMBUS20: 0.2,
}

export const useCartStore = defineStore('cart', {
  state: () => ({
    items: [],
    promoCode: '',
  }),

  getters: {
    count(state) {
      return state.items.reduce((sum, item) => sum + item.qty, 0)
    },

    subtotal(state) {
      return state.items.reduce((sum, item) => sum + item.price * item.qty, 0)
    },

    discount(state) {
      return Math.round(state.subtotal * (PROMO_CODES[state.promoCode] ?? 0))
    },

    total() {
      return this.subtotal - this.discount
    },
  },

  actions: {
    add(product) {
      const existing = this.items.find((item) => item.id === product.id)
      if (existing) {
        existing.qty += 1
      } else {
        this.items.push({ id: product.id, name: product.name, price: product.price, qty: 1 })
      }
    },

    remove(productId) {
      this.items = this.items.filter((item) => item.id !== productId)
    },

    setQty(productId, qty) {
      const item = this.items.find((i) => i.id === productId)
      if (item) item.qty = Math.max(1, qty)
    },

    applyPromo(code) {
      this.promoCode = code.trim().toUpperCase()
    },

    clear() {
      this.items = []
      this.promoCode = ''
    },
  },
})