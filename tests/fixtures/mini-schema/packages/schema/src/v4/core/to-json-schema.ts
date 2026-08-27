export function toJsonSchema(schema: object) {
    return { type: "object", schema };
}

export function parseJsonSchema(schema: object) {
    return toJsonSchema(schema);
}
