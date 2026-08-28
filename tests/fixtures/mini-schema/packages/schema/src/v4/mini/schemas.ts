/** Trimmed v4 mini surface — decoy for generic parse/parsing prompts. */
export function parse(input: unknown) {
  return { success: true, data: input };
}

export function safeParse(input: unknown) {
  return parse(input);
}
