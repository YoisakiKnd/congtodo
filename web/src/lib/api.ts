export class ApiError extends Error {
  constructor(
    public status: number,
    message: string,
  ) {
    super(message);
  }
}

/** 会话失效时的回调（由 store 注册，用于切回登录页） */
let onUnauthorized = () => {};
export const setUnauthorizedHandler = (fn: () => void) => (onUnauthorized = fn);

// 会话放在 HttpOnly Cookie 里，同源请求会自动携带
async function request<T>(method: string, path: string, body?: unknown): Promise<T> {
  const res = await fetch(`/api${path}`, {
    method,
    headers: body === undefined ? undefined : { 'content-type': 'application/json' },
    body: body === undefined ? undefined : JSON.stringify(body),
  });
  if (res.status === 204) return undefined as T;
  const data = await res.json().catch(() => ({}));
  if (!res.ok) {
    if (res.status === 401 && !path.startsWith('/auth/')) onUnauthorized();
    throw new ApiError(res.status, data.error ?? res.statusText);
  }
  return data as T;
}

/** 上传图片（原始二进制），返回文件 id */
async function upload(blob: Blob): Promise<number> {
  const res = await fetch('/api/files', { method: 'POST', headers: { 'content-type': blob.type }, body: blob });
  const data = await res.json().catch(() => ({}));
  if (!res.ok) throw new ApiError(res.status, data.error ?? res.statusText);
  return data.id;
}

export const api = {
  upload,
  get: <T>(p: string) => request<T>('GET', p),
  post: <T>(p: string, body?: unknown) => request<T>('POST', p, body ?? {}),
  put: <T>(p: string, body: unknown) => request<T>('PUT', p, body),
  patch: <T>(p: string, body: unknown) => request<T>('PATCH', p, body),
  del: (p: string) => request<void>('DELETE', p),
};
