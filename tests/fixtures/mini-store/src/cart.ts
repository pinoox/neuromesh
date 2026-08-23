import { createStore } from "./store";

export function addToCart(item: string, qty: number) {
    const store = createStore({ [item]: 0 });
    const next = store.get(item) + qty;
    store.set(item, next);
    return next;
}

export function unusedLogger() {
    console.log("a");
    console.log("b");
    console.log("c");
    console.log("d");
}
