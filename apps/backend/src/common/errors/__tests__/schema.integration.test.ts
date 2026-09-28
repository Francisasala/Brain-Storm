import { describe, it, expect } from 'vitest';
import { ErrorCodes, toErrorResponse } from '@brain-storm/types/src/error.types';
import { mapContractError, CONTRACT_ERROR_MAP } from '../contract-error.map';
import { normalizeError, statusForCode } from '../unified-error.adapter';

describe('error schema (#1212)', () => {
  it('toErrorResponse returns schema-shaped object', () => {
    expect(toErrorResponse(ErrorCodes.NOT_FOUND, 'User not found')).toEqual({
      code: 'NOT_FOUND',
      message: 'User not found',
    });
  });

  it('toErrorResponse includes details and requestId', () => {
    expect(toErrorResponse(ErrorCodes.VALIDATION_FAILED, 'bad', { f: 'email' }, 'r-1')).toEqual({
      code: 'VALIDATION_FAILED',
      message: 'bad',
      details: { f: 'email' },
      requestId: 'r-1',
    });
  });
});

describe('contract error mapping (#1212)', () => {
  it('maps known codes', () => {
    const r = mapContractError({ code: 2 });
    expect(r.code).toBe(ErrorCodes.INSUFFICIENT_BALANCE);
    expect(r.details).toMatchObject({ contractCode: 2 });
  });

  it('every map entry has code + message', () => {
    for (const [k, v] of Object.entries(CONTRACT_ERROR_MAP)) {
      expect(typeof v.code).toBe('string');
      expect(v.message.length).toBeGreaterThan(0);
      expect(Number(k)).toBeGreaterThan(0);
    }
  });

  it('unknown codes fall back to INTERNAL_ERROR', () => {
    expect(mapContractError({ code: 9999 }).code).toBe(ErrorCodes.INTERNAL_ERROR);
  });
});

describe('normalizeError (#1212)', () => {
  it('passes schema-shaped errors through', () => {
    expect(normalizeError({ code: 'CONFLICT', message: 'exists' })).toMatchObject({
      code: 'CONFLICT',
      message: 'exists',
    });
  });

  it('wraps plain Error', () => {
    const r = normalizeError(new Error('boom'));
    expect(r.code).toBe('INTERNAL_ERROR');
    expect(r.message).toBe('boom');
  });

  it('wraps unknown values safely', () => {
    const r = normalizeError('some string');
    expect(r.code).toBe('INTERNAL_ERROR');
    expect(typeof r.message).toBe('string');
  });
});

describe('statusForCode (#1212)', () => {
  it.each([
    ['VALIDATION_FAILED', 400],
    ['UNAUTHORIZED', 401],
    ['NOT_FOUND', 404],
    ['CONFLICT', 409],
    ['INSUFFICIENT_BALANCE', 422],
    ['RATE_LIMITED', 429],
    ['INTERNAL_ERROR', 500],
  ])('maps %s -> %i', (code, status) => {
    expect(statusForCode(code)).toBe(status);
  });
});
