const TOKEN_KEY = 'congtodo.token';

export const token = {
  get(): string {
    try {
      return localStorage.getItem(TOKEN_KEY) ?? '';
    } catch {
      return '';
    }
  },
  set(v: string) {
    try {
      localStorage.setItem(TOKEN_KEY, v);
    } catch {}
  },
};

export class ApiError extends Error {
  constructor(
    public status: number,
    message: string,
  ) {
    super(message);
  }
}

async function request<T>(method: string, path: string, body?: unknown): Promise<T> {
  const headers: Record<string, string> = {};
  if (body !== undefined) headers['content-type'] = 'application/json';
  const t = token.get();
  if (t) headers.authorization = `Bearer ${t}`;

  const res = await fetch(`/api${path}`, {
    method,
    headers,
    body: body === undefined ? undefined : JSON.stringify(body),
  });
  if (res.status === 204) return undefined as T;
  const data = await res.json().catch(() => ({}));
  if (!res.ok) throw new ApiError(res.status, res.status === 401 ? '未授权：请在设置里填写访问令牌' : (data.error ?? res.statusText));
  return data as T;
}

export const api = {
  get: <T>(p: string) => request<T>('GET', p),
  post: <T>(p: string, body?: unknown) => request<T>('POST', p, body ?? {}),
  put: <T>(p: string, body: unknown) => request<T>('PUT', p, body),
  patch: <T>(p: string, body: unknown) => request<T>('PATCH', p, body),
  del: (p: string) => request<void>('DELETE', p),
};
