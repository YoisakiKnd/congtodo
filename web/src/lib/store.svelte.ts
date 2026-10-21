import { SvelteMap } from 'svelte/reactivity';
import { toast } from 'svelte-sonner';
import { api, ApiError, setUnauthorizedHandler } from './api';
import { addDays, ymd } from './time';
import type { CalEvent, Channel, Countdown, Course, Feed, FocusSession, Habit, HabitLog, Holiday, Todo, TodoList } from './types';

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

const fail = (e: unknown): never => {
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

  /** 返回服务端的最新数据（例如重复待办完成后会带 `_next: true`） */
  async patch(id: number, data: Partial<T>): Promise<T & { _next?: boolean }> {
    const i = this.items.findIndex((x) => x.id === id);
    const old = this.items[i];
    if (old) this.items[i] = { ...old, ...data };
    try {
      const row = await api.patch<T & { _next?: boolean }>(`/${this.name}/${id}`, data);
      const j = this.items.findIndex((x) => x.id === id);
      if (j >= 0) this.items[j] = row;
      return row;
    } catch (e) {
      if (old) this.items[i] = old;
      return fail(e);
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
  feeds: new Collection<Feed>('feeds'),
  focus: new Collection<FocusSession>('focus_sessions'),
};

// ---------- 设置（key-value） ----------

export const settings = $state<Record<string, string>>({});

export async function saveSettings(patch: Record<string, string>) {
  await api.put('/settings', patch).catch(fail);
  Object.assign(settings, patch);
}

// ---------- 习惯打卡记录："habitId:YYYY-MM-DD" -> { count, note } ----------

export const habitLogs = new SvelteMap<string, HabitLog>();
export const LOG_DAYS = 400;

export const habitLog = (id: number, day: string) => habitLogs.get(`${id}:${day}`);
export const habitCount = (id: number, day: string) => habitLog(id, day)?.count ?? 0;
/** 当天是否达成（计数型习惯需达到 goal 次） */
export const habitDone = (h: Habit, day: string) => habitCount(h.id, day) >= (h.goal || 1);

/** 设置某天的次数和备注；次数为 0 且无备注会删除记录 */
export async function logHabit(id: number, day: string, count: number, note: string | null = habitLog(id, day)?.note ?? null) {
  const key = `${id}:${day}`;
  const old = habitLogs.get(key);
  count = Math.max(0, count);
  if (count === 0 && !note) habitLogs.delete(key);
  else habitLogs.set(key, { count, note });
  await api.post(`/habits/${id}/log`, { day, count, note }).catch((e) => {
    old ? habitLogs.set(key, old) : habitLogs.delete(key);
    fail(e);
  });
}

/** 点一下：未达成则 +1（单次习惯即完成），已达成则清零 */
export function toggleHabit(h: Habit, day: string) {
  return logHabit(h.id, day, habitDone(h, day) ? 0 : habitCount(h.id, day) + 1);
}

// ---------- 待办清单（含别人分享给我的） ----------

export const lists = $state({ items: [] as TodoList[] });

export async function loadLists() {
  lists.items = await api.get<TodoList[]>('/lists').catch(fail);
}

export async function saveList(data: { id?: number; name?: string; color?: string | null }) {
  const row = data.id
    ? await api.patch<TodoList>(`/lists/${data.id}`, data).catch(fail)
    : await api.post<TodoList>('/lists', data).catch(fail);
  const i = lists.items.findIndex((l) => l.id === row.id);
  if (i >= 0) lists.items[i] = row;
  else lists.items.push(row);
  return row;
}

export async function deleteList(id: number) {
  await api.del(`/lists/${id}`).catch(fail);
  lists.items = lists.items.filter((l) => l.id !== id);
  // 清单里的待办会变成「无清单」，重新拉取一次
  await db.todos.load();
}

export async function shareList(id: number, username: string) {
  const row = await api.post<TodoList>(`/lists/${id}/members`, { username }).catch(fail);
  lists.items = lists.items.map((l) => (l.id === id ? row : l));
}

export async function removeMember(listId: number, userId: number) {
  await api.del(`/lists/${listId}/members/${userId}`).catch(fail);
  await Promise.all([loadLists(), db.todos.load()]);
}

// ---------- 法定节假日（按年懒加载） ----------

export const holidays = new SvelteMap<string, Holiday>();
const holidayYears = new Set<number>();

export function ensureHolidays(year: number) {
  if (holidayYears.has(year)) return;
  holidayYears.add(year);
  api
    .get<{ date: string; name: string; off: boolean }[]>(`/holidays?year=${year}`)
    .then((rows) => rows.forEach((r) => holidays.set(r.date, { name: r.name, off: r.off })))
    .catch(() => holidayYears.delete(year));
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
  lists.items = [];
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

/** 浏览器所在时区 */
export const browserTimezone = () => Intl.DateTimeFormat().resolvedOptions().timeZone;

export async function loadAll() {
  const from = ymd(addDays(new Date(), -LOG_DAYS));
  await Promise.allSettled([
    ...Object.values(db).map((c) => c.load()),
    loadLists(),
    api.get<Record<string, string>>('/settings').then((s) => Object.assign(settings, s)),
    api.get<{ habit_id: number; day: string; count: number; note: string | null }[]>(`/habit_logs?from=${from}`).then((rows) => {
      habitLogs.clear();
      for (const r of rows) habitLogs.set(`${r.habit_id}:${r.day}`, { count: r.count, note: r.note });
    }),
  ]);
  ensureHolidays(new Date().getFullYear());
  // 第一次登录时把浏览器时区保存为账号时区，服务端按它计算提醒时间
  if (!settings.timezone && browserTimezone()) saveSettings({ timezone: browserTimezone() }).catch(() => {});
}

export const PALETTE = ['#5b5bd6', '#e5484d', '#f76b15', '#f5a524', '#30a46c', '#12a594', '#0091ff', '#d6409f', '#8e4ec6', '#78716c'];
