export class ZodError extends Error {
    path: (string | number)[] = [];
}

export function parse(schema: object, data: unknown) {
    return data;
}

export function safeParse(schema: object, data: unknown) {
    return { success: true, data };
}
