import { createApp } from 'vue';
import PrimeVue from 'primevue/config';
import { createPinia } from 'pinia';
import Dashboard from './Dashboard.vue';

const app = createApp(Dashboard);
app.use(createPinia());
app.use(PrimeVue);
app.mount('#app');
