import { defineStore } from 'pinia';

export const useDashboardStore = defineStore('dashboard', {
  state: () => ({ rows: [] as { id: number; label: string }[] }),
  actions: {
    refresh() {
      this.rows = [{ id: 1, label: 'inbox' }];
    },
  },
});
