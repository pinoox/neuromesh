export type Issue = { path: (string | number)[]; message: string };

export function parse(schema: object, data: unknown) {
    const result = _parse(schema, data, []);
    if (result.issues.length > 0) {
        throw result;
    }
    return result.value;
}

export function safeParse(schema: object, data: unknown) {
    return _parse(schema, data, []);
}

function _parse(schema: object, data: unknown, path: (string | number)[]) {
    const issues: Issue[] = [];
    if (typeof data !== "object" || data === null) {
        issues.push({ path, message: "invalid_type" });
    }
    return { value: data, issues };
}

export function _safeParse(schema: object, data: unknown) {
    return safeParse(schema, data);
}
