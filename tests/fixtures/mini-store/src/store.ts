export function createStore(initial: Record<string, number>) {
    const state = { ...initial };
    return {
        get(key: string) {
            return state[key] ?? 0;
        },
        set(key: string, value: number) {
            state[key] = value;
        },
    };
}
