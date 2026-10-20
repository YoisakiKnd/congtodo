import { SvelteSet } from 'svelte/reactivity';
import { api } from './api';
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

export const toast = $state({ msg: '' });
let toastTimer: ReturnType<typeof setTimeout>;
export function notify(msg: string) {
  toast.msg = msg;
  clearTimeout(toastTimer);
  toastTimer = setTimeout(() => (toast.msg = ''), 2800);
}
const fail = (e: unknown) => {
  notify(e instanceof Error ? e.message : String(e));
  throw e;
};

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

// ---------- 启动加载 ----------

export const ready = $state({ ok: false });

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
  ready.ok = true;
}

export const PALETTE = ['#5b5bd6', '#e5484d', '#f76b15', '#f5a524', '#30a46c', '#12a594', '#0091ff', '#d6409f', '#8e4ec6', '#78716c'];
