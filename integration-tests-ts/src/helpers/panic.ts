import type {TransactionResult} from 'near-workspaces';

export function hasPanic(result: TransactionResult, message: string): boolean {
  return JSON.stringify(result).includes(message);
}

export async function expectPanic(
  call: Promise<TransactionResult>,
  message: string,
): Promise<boolean> {
  try {
    const result = await call;
    return hasPanic(result, message);
  } catch (err) {
    const text = err instanceof Error ? err.message : String(err);
    return text.includes(message);
  }
}
