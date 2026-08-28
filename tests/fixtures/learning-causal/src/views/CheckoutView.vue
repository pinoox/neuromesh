<script setup>
import { useCartStore } from '../stores/cart.js'
const cart = useCartStore()
</script>
<template><div>{{ cart.qty }}</div></template>
