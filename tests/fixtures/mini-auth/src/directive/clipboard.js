export function clipboard(el, binding) {
  const text = String(binding.value);
  el.setAttribute("data-clipboard", text);
  return text;
}
