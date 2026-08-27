function createStore(initial) {
    const state = { ...initial };
    return {
        get(key) {
            return state[key] ?? 0;
        },
        set(key, value) {
            state[key] = value;
        },
    };
}

module.exports = { createStore };
