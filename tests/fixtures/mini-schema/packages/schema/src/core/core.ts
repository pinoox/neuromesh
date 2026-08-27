export type output<T> = T extends { _output: infer Out } ? Out : T;
