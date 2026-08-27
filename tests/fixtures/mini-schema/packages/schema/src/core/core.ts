export type ZodType<T = unknown> = { _output: T; _input: T };

export type output<T> = T extends { _output: infer Out } ? Out : T;

export type input<T> = T extends { _input: infer In } ? In : T;
