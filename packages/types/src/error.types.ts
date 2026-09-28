export enum ErrorCode {
  VALIDATION_ERROR = 'VALIDATION_ERROR',
  NOT_FOUND = 'NOT_FOUND',
  UNAUTHORIZED = 'UNAUTHORIZED',
  FORBIDDEN = 'FORBIDDEN',
  CONFLICT = 'CONFLICT',
  INTERNAL_ERROR = 'INTERNAL_ERROR',
  EXTERNAL_SERVICE_ERROR = 'EXTERNAL_SERVICE_ERROR',
  RATE_LIMIT_EXCEEDED = 'RATE_LIMIT_EXCEEDED',
}

export interface AppErrorResponse {
  statusCode: number;
  code: ErrorCode;
  message: string;
  context?: Record<string, any>;
  timestamp: string;
}

export interface ValidationErrorDetail {
  field: string;
  message: string;
  value?: any;
}

export interface ErrorContext {
  [key: string]: any;
}

/**
 * Shared error-response schema (#1212).
 *
 * Every backend HTTP error and every contract error surfaced through the
 * API MUST be shaped as ErrorResponse.
 */
export interface ErrorResponse {
  code: string;
  message: string;
  details?: unknown;
  requestId?: string;
}

export const ErrorCodes = {
  VALIDATION_FAILED: 'VALIDATION_FAILED',
  UNAUTHORIZED: 'UNAUTHORIZED',
  UNAUTHORIZED_CALLER: 'UNAUTHORIZED_CALLER',
  FORBIDDEN: 'FORBIDDEN',
  NOT_FOUND: 'NOT_FOUND',
  CONFLICT: 'CONFLICT',
  RATE_LIMITED: 'RATE_LIMITED',
  INSUFFICIENT_BALANCE: 'INSUFFICIENT_BALANCE',
  INVALID_STATE: 'INVALID_STATE',
  ARITHMETIC_OVERFLOW: 'ARITHMETIC_OVERFLOW',
  INTERNAL_ERROR: 'INTERNAL_ERROR',
} as const;

export type ErrorCode = (typeof ErrorCodes)[keyof typeof ErrorCodes];

export function toErrorResponse(
  code: string,
  message: string,
  details?: unknown,
  requestId?: string,
): ErrorResponse {
  const out: ErrorResponse = { code, message };
  if (details !== undefined) out.details = details;
  if (requestId !== undefined) out.requestId = requestId;
  return out;
}
