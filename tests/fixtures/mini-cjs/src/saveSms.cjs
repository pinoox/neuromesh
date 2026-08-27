const { createStore } = require("./store");

function saveSms(body) {
    const store = createStore({ body: 0 });
    return store.get("body");
}

module.exports = { saveSms };
