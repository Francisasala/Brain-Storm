/**
 * Maps Soroban/contract error codes (u32) to the shared ErrorResponse schema
 * at the backend boundary. Update whenever `contracts/shared/src/errors.rs`
 * adds a new variant. Closes #1212.
 */
import {
  ErrorResponse,
  ErrorCodes,
  toErrorResponse,
} from '@brain-storm/types/src/error.types';

/**
 * Numeric codes mirror the on-chain enum variants 1:1.
 * Source of truth: contracts/shared/src/errors.rs
 */
export const CONTRACT_ERROR_MAP: Record<number, { code: string; message: string }> = {
  1: { code: ErrorCodes.UNAUTHORIZED_CALLER, message: 'Caller is not authorized for this operation' },
  2: { code: ErrorCodes.INSUFFICIENT_BALANCE, message: 'Insufficient balance for this operation' },
  3: { code: ErrorCodes.INVALID_STATE, message: 'Contract is in an invalid state for this operation' },
  4: { code: ErrorCodes.ARITHMETIC_OVERFLOW, message: 'Arithmetic overflow' },
  5: { code: ErrorCodes.VALIDATION_FAILED, message: 'Invalid input' },
};

export function mapContractError(err: unknown, requestId?: string): ErrorResponse {
  const candidates: Array<number | string | undefined> = [
    (err as { code?: number | string })?.code,
    (err as { errorCode?: number })?.errorCode,
    (err as { result?: { code?: number } })?.result?.code,
  ];

  for (const c of candidates) {
    const n = typeof c === 'string' ? parseInt(c, 10) : c;
    if (typeof n === 'number' && !Number.isNaN(n) && CONTRACT_ERROR_MAP[n]) {
      const entry = CONTRACT_ERROR_MAP[n];
      return toErrorResponse(entry.code, entry.message, { contractCode: n }, requestId);
    }
  }

  const message = err instanceof Error ? err.message : 'Contract call failed';
  return toErrorResponse(ErrorCodes.INTERNAL_ERROR, message, { original: String(err) }, requestId);
}
