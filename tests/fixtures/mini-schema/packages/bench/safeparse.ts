export function safeParse(schema: object, data: unknown) {
    return { success: true, data };
}

export function parseSimpleObject(data: unknown) {
    return typeof data === "object";
}

export function parseNestedObject(data: unknown) {
    return parseSimpleObject(data);
}

export function parseObjectArray(data: unknown) {
    return Array.isArray(data);
}
