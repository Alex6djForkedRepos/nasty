import { afterEach, describe, expect, test, vi } from 'vitest';
import { EngineUnavailableError, login } from './auth';

afterEach(() => vi.unstubAllGlobals());

describe('password login errors', () => {
	test('returns after a successful response', async () => {
		vi.stubGlobal('fetch', vi.fn().mockResolvedValue(new Response('{}', { status: 200 })));
		await expect(login('admin', 'secret')).resolves.toBeUndefined();
	});

	test('preserves credential errors from the engine', async () => {
		vi.stubGlobal('fetch', vi.fn().mockResolvedValue(
			new Response(JSON.stringify({ error: 'invalid credentials' }), {
				status: 401,
				headers: { 'Content-Type': 'application/json' },
			}),
		));
		await expect(login('admin', 'wrong')).rejects.toThrow('invalid credentials');
	});

	test.each([502, 503, 504])('classifies HTTP %s as engine startup', async (status) => {
		vi.stubGlobal('fetch', vi.fn().mockResolvedValue(new Response('', { status })));
		await expect(login('admin', 'secret')).rejects.toBeInstanceOf(EngineUnavailableError);
	});

	test('distinguishes a network failure from an HTTP response', async () => {
		vi.stubGlobal('fetch', vi.fn().mockRejectedValue(new TypeError('connection refused')));
		await expect(login('admin', 'secret')).rejects.toThrow("Can't reach server: connection refused");
	});
});
