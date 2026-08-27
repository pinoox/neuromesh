export function localeError(issue: { path: unknown[]; code: string }) {
    if (issue.code === "invalid_type") {
        return "validation error at path";
    }
    return "invalid";
}

export function invalidTypeError() {
    return "invalid type";
}
