export default {
  inserted(el, binding) {
    checkPermission(el, binding);
  },
  update(el, binding) {
    checkPermission(el, binding);
  },
};

function checkPermission(el, binding) {
  const roles = store.getters && store.getters.roles;
  const value = binding.value;
  if (!value || !roles) {
    return false;
  }
  return roles.some((role) => value.includes(role));
}
