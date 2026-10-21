// 专注计时（番茄钟）：全局单例，悬浮小窗和专注面板共用。
// 状态存 localStorage，刷新页面不丢；endsAt 是绝对时间，后台标签页也准。
import { toast } from 'svelte-sonner';
import { clock, db, settings } from './store.svelte';
import { toTs } from './time';

export type FocusMode = 'focus' | 'short' | 'long';
export const FOCUS_LABEL: Record<FocusMode, string> = { focus: '专注', short: '短休息', long: '长休息' };
const DEFAULTS: Record<FocusMode, number> = { focus: 25, short: 5, long: 15 };
export const focusMinutes = (m: FocusMode) => Number(settings[`focus_${m}`]) || DEFAULTS[m];

type Timer = {
  mode: FocusMode;
  endsAt: number | null;
  remaining: number;
  startedAt: number | null;
  todoId: number | null;
  rounds: number;
};

const KEY = 'congtodo.focus';
function load(): Timer {
  try {
    const t = JSON.parse(localStorage.getItem(KEY) ?? '');
    if (t?.mode) return t;
  } catch {}
  return { mode: 'focus', endsAt: null, remaining: DEFAULTS.focus * 60, startedAt: null, todoId: null, rounds: 0 };
}

export const timer = $state<Timer>(load());
const persist = () => {
  try {
    localStorage.setItem(KEY, JSON.stringify(timer));
  } catch {}
};

export const focus = {
  get running() {
    return timer.endsAt != null;
  },
  /** 剩余秒数 */
  get left() {
    return timer.endsAt != null ? Math.max(0, Math.round((timer.endsAt - clock.now) / 1000)) : timer.remaining;
  },
  get total() {
    return focusMinutes(timer.mode) * 60;
  },
  /** 计时器是否在使用中（运行中或暂停在中途） */
  get active() {
    return timer.endsAt != null || timer.startedAt != null;
  },
};

export function switchMode(m: FocusMode) {
  Object.assign(timer, { mode: m, endsAt: null, remaining: focusMinutes(m) * 60, startedAt: null });
  persist();
}
export const resetFocus = () => switchMode(timer.mode);

export function startFocus(todoId?: number | null) {
  // 指定了待办：切到专注模式并关联它
  if (todoId !== undefined) {
    if (timer.mode !== 'focus' || !focus.active) switchMode('focus');
    timer.todoId = todoId;
  }
  if (!focus.running) {
    timer.startedAt ??= Math.floor(Date.now() / 1000);
    timer.endsAt = Date.now() + timer.remaining * 1000;
  }
  persist();
  if ('Notification' in window && Notification.permission === 'default') Notification.requestPermission();
}

export function pauseFocus() {
  if (!focus.running) return;
  timer.remaining = focus.left;
  timer.endsAt = null;
  persist();
}

export function setFocusTodo(id: number | null) {
  timer.todoId = id;
  persist();
}

function beep() {
  try {
    const ctx = new AudioContext();
    [0, 0.25, 0.5].forEach((t) => {
      const o = ctx.createOscillator();
      const g = ctx.createGain();
      o.frequency.value = 880;
      g.gain.setValueAtTime(0.2, ctx.currentTime + t);
      g.gain.exponentialRampToValueAtTime(0.001, ctx.currentTime + t + 0.2);
      o.connect(g).connect(ctx.destination);
      o.start(ctx.currentTime + t);
      o.stop(ctx.currentTime + t + 0.2);
    });
  } catch {}
}

let finishing = false;

/** 结束当前阶段：专注阶段记一条专注记录；skipped = 手动跳过（不响铃） */
export async function finishFocus(skipped = false) {
  if (finishing) return;
  finishing = true;
  try {
    const mode = timer.mode;
    if (mode === 'focus') {
      const minutes = Math.round((focus.total - focus.left) / 60);
      if (minutes >= 1) {
        await db.focus.add({ todo_id: timer.todoId, started_at: timer.startedAt ?? toTs(new Date()) - minutes * 60, minutes });
      }
      if (!skipped) timer.rounds += 1;
    }
    if (!skipped) {
      beep();
      const msg = mode === 'focus' ? '专注完成，休息一下吧 ☕' : '休息结束，继续专注 💪';
      toast.success(msg);
      if ('Notification' in window && Notification.permission === 'granted') new Notification('Congtodo', { body: msg });
    }
    // 每 4 个番茄来一次长休息
    switchMode(mode === 'focus' ? (timer.rounds && timer.rounds % 4 === 0 ? 'long' : 'short') : 'focus');
  } finally {
    finishing = false;
  }
}

/** 彻底停止（回到专注模式初始状态，不记录） */
export function stopFocus() {
  timer.todoId = null;
  switchMode('focus');
}
