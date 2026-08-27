<script setup>
import Button from 'primevue/button';
import DataTable from 'primevue/datatable';
import { useDashboardStore } from './stores/dashboard';

const store = useDashboardStore();
</script>

<template>
  <div>
    <Button label="Refresh" @click="store.refresh" />
    <DataTable :value="store.rows" />
    <data-table :value="store.rows" />
  </div>
</template>
