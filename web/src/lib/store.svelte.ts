import { SvelteSet } from 'svelte/reactivity';
import { toast } from 'svelte-sonner';
import { api, ApiError, setUnauthorizedHandler } from './api';
import { addDays, ymd } from './time';
import type { CalEvent, Channel, Countdown, Course, Habit, Todo } from './types';

// ---------- 时钟：now 每秒更新，minute 只在分钟变化时更新，避免无谓重渲染 ----------

export const clock = $state({ now: Date.now(), minute: Math.floor(Date.now() / 60_000) * 60_000 });
setInterval(() => {
  clock.now = Date.now();
  const m = Math.floor(clock.now / 60_000) * 60_000;
  if (m !== clock.minute) clock.minute = m;
}, 1000);

// ---------- 路由（hash） ----------

const readHash = () => location.hash.slice(1) || '/';
export const route = $state({ path: readHash() });
addEventListener('hashchange', () => (route.path = readHash()));

// ---------- 提示 ----------

const fail = (e: unknown) => {
  // 会话失效会直接切回登录页，不再逐条弹错
  if (!(e instanceof ApiError && e.status === 401)) toast.error(e instanceof Error ? e.message : String(e));
  throw e;
};
// fail 重新抛出是为了让调用方（如表单对话框）知道失败；没人接的 API 错误已经提示过，不再报未处理异常
addEventListener('unhandledrejection', (e) => e.reason instanceof ApiError && e.preventDefault());

// ---------- 通用集合：乐观更新，失败回滚 ----------

export class Collection<T extends { id: number }> {
  items = $state<T[]>([]);

  constructor(private name: string) {}

  async load() {
    this.items = await api.get<T[]>(`/${this.name}`).catch(fail);
  }

  async add(data: Partial<T>): Promise<T> {
    const row = await api.post<T>(`/${this.name}`, data).catch(fail);
    this.items.push(row);
    return row;
  }

  async patch(id: number, data: Partial<T>) {
    const i = this.items.findIndex((x) => x.id === id);
    const old = this.items[i];
    if (old) this.items[i] = { ...old, ...data };
    try {
      const row = await api.patch<T>(`/${this.name}/${id}`, data);
      const j = this.items.findIndex((x) => x.id === id);
      if (j >= 0) this.items[j] = row;
    } catch (e) {
      if (old) this.items[i] = old;
      fail(e);
    }
  }

  async remove(id: number) {
    const old = this.items;
    this.items = old.filter((x) => x.id !== id);
    await api.del(`/${this.name}/${id}`).catch((e) => {
      this.items = old;
      fail(e);
    });
  }

  /** 有 id 则更新，否则新建 */
  save(data: Partial<T>) {
    return data.id ? this.patch(data.id, data) : this.add(data);
  }
}

export const db = {
  countdowns: new Collection<Countdown>('countdowns'),
  events: new Collection<CalEvent>('events'),
  courses: new Collection<Course>('courses'),
  todos: new Collection<Todo>('todos'),
  habits: new Collection<Habit>('habits'),
  channels: new Collection<Channel>('channels'),
};

// ---------- 设置（key-value） ----------

export const settings = $state<Record<string, string>>({});

export async function saveSettings(patch: Record<string, string>) {
  await api.put('/settings', patch).catch(fail);
  Object.assign(settings, patch);
}

// ---------- 习惯打卡记录："habitId:YYYY-MM-DD" ----------

export const habitLogs = new SvelteSet<string>();
export const LOG_DAYS = 180;

export async function toggleHabit(id: number, day: string) {
  const key = `${id}:${day}`;
  const had = habitLogs.has(key);
  had ? habitLogs.delete(key) : habitLogs.add(key);
  await api.post(`/habits/${id}/toggle`, { day }).catch((e) => {
    had ? habitLogs.add(key) : habitLogs.delete(key);
    fail(e);
  });
}

// ---------- 账号 ----------

export type User = { id: number; username: string; is_admin: boolean };

export const auth = $state({
  user: null as User | null,
  checked: false, // 已确认登录状态（避免闪现登录页）
  signup: false, // 当前是否开放注册
  first: false, // 还没有任何用户：注册的将是管理员
});

async function loadAuthConfig() {
  const c = await api.get<{ signup: boolean; first: boolean }>('/auth/config');
  auth.signup = c.signup;
  auth.first = c.first;
}

function resetData() {
  for (const c of Object.values(db)) c.items = [];
  habitLogs.clear();
  for (const k of Object.keys(settings)) delete settings[k];
}

setUnauthorizedHandler(() => {
  if (!auth.user) return;
  auth.user = null;
  resetData();
  toast.info('登录已过期，请重新登录');
  loadAuthConfig();
});

export async function checkAuth() {
  try {
    auth.user = await api.get<User>('/auth/me');
  } catch {
    auth.user = null;
    await loadAuthConfig().catch(() => {});
  }
  auth.checked = true;
  if (auth.user) await loadAll();
}

export async function signIn(username: string, password: string, register = false) {
  auth.user = await api.post<User>(register ? '/auth/register' : '/auth/login', { username, password });
  await loadAll();
}

export async function signOut() {
  await api.post('/auth/logout').catch(() => {});
  auth.user = null;
  resetData();
  await loadAuthConfig().catch(() => {});
}

export const changePassword = (old: string, next: string) => api.post('/auth/password', { old, new: next });

// ---------- 加载数据 ----------

export async function loadAll() {
  const from = ymd(addDays(new Date(), -LOG_DAYS));
  await Promise.allSettled([
    ...Object.values(db).map((c) => c.load()),
    api.get<Record<string, string>>('/settings').then((s) => Object.assign(settings, s)),
    api.get<{ habit_id: number; day: string }[]>(`/habit_logs?from=${from}`).then((rows) => {
      habitLogs.clear();
      for (const r of rows) habitLogs.add(`${r.habit_id}:${r.day}`);
    }),
  ]);
}

export const PALETTE = ['#5b5bd6', '#e5484d', '#f76b15', '#f5a524', '#30a46c', '#12a594', '#0091ff', '#d6409f', '#8e4ec6', '#78716c'];
