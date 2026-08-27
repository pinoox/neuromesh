export function validateSimpleObject(data: unknown) {
    return typeof data === "object" && data !== null;
}

export function parseSimpleObject(data: unknown) {
    if (!validateSimpleObject(data)) {
        return { success: false };
    }
    return { success: true, data };
}
