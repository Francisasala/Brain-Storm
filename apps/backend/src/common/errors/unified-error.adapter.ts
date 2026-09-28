/**
 * Adapter that normalizes any thrown value into the shared ErrorResponse
 * schema (#1212). Use from the existing global exception filter.
 */
import { ErrorResponse, ErrorCodes, toErrorResponse } from '@brain-storm/types/src/error.types';
import { mapContractError } from './contract-error.map';

export function statusForCode(code: string): number {
  switch (code) {
    case ErrorCodes.VALIDATION_FAILED: return 400;
    case ErrorCodes.UNAUTHORIZED:
    case ErrorCodes.UNAUTHORIZED_CALLER: return 401;
    case ErrorCodes.FORBIDDEN: return 403;
    case ErrorCodes.NOT_FOUND: return 404;
    case ErrorCodes.CONFLICT: return 409;
    case ErrorCodes.INSUFFICIENT_BALANCE:
    case ErrorCodes.INVALID_STATE: return 422;
    case ErrorCodes.RATE_LIMITED: return 429;
    case ErrorCodes.ARITHMETIC_OVERFLOW:
    case ErrorCodes.INTERNAL_ERROR:
    default: return 500;
  }
}

export function normalizeError(err: unknown, requestId?: string): ErrorResponse {
  if (err && typeof err === 'object' && 'code' in err && 'message' in err) {
    const e = err as { code: unknown; message: unknown; details?: unknown };
    if (typeof e.code === 'string' && typeof e.message === 'string') {
      return toErrorResponse(e.code, e.message, e.details, requestId);
    }
  }

  if (err && typeof err === 'object' && ('errorCode' in err || 'result' in err)) {
    return mapContractError(err, requestId);
  }

  if (err instanceof Error) {
    return toErrorResponse(ErrorCodes.INTERNAL_ERROR, err.message, undefined, requestId);
  }

  return toErrorResponse(
    ErrorCodes.INTERNAL_ERROR,
    'Unknown error',
    { original: String(err) },
    requestId,
  );
}
