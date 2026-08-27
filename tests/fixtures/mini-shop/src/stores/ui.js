import { defineStore } from 'pinia'

export const useUiStore = defineStore('ui', {
  state: () => ({
    currentView: 'home', // home | product | cart
    selectedProductId: null,
    cartOpen: false,
    toast: null,
  }),

  actions: {
    openProduct(id) {
      this.selectedProductId = id
      this.currentView = 'product'
    },

    openCart() {
      this.cartOpen = true
    },

    closeCart() {
      this.cartOpen = false
    },

    goCheckout() {
      this.cartOpen = false
      this.currentView = 'checkout'
    },

    goCart() {
      this.currentView = 'cart'
    },

    showToast(message) {
      this.toast = { message, id: Date.now() }
      setTimeout(() => {
        this.toast = null
      }, 2500)
    },
  },
})