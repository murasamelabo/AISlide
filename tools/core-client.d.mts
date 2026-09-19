export const MAX_REQUEST_BYTES: number;
export function coreTimeout(request: unknown, encodedBytes?: number): number;
export function requestCore(request: unknown, options?: { signal?: AbortSignal }): Promise<unknown>;