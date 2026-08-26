export function hasPermission(roles, routeRoles) {
  if (!routeRoles || routeRoles.length === 0) {
    return true;
  }
  return roles.some((role) => routeRoles.includes(role));
}

export function generateRoutes(roles) {
  return filterAsyncRoutes(asyncRoutes, roles);
}

function filterAsyncRoutes(routes, roles) {
  return routes.filter((route) => hasPermission(roles, route.meta && route.meta.roles));
}
