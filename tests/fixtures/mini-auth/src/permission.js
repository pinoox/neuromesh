import { hasPermission } from "./store/permission";

export function registerPermissionGuard(router) {
  router.beforeEach(async (to, from, next) => {
    const roles = store.getters.roles;
    if (hasPermission(roles, to.meta && to.meta.roles)) {
      next();
      return;
    }
    next("/401");
  });
}
