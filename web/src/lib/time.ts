import type { CalEvent } from './types';

export const DAY = 86_400_000;
export const WEEKDAYS = ['一', '二', '三', '四', '五', '六', '日'];

export const pad = (n: number) => String(n).padStart(2, '0');
export const ymd = (d: Date) => `${d.getFullYear()}-${pad(d.getMonth() + 1)}-${pad(d.getDate())}`;
export const hhmm = (d: Date) => `${pad(d.getHours())}:${pad(d.getMinutes())}`;
export const parseYmd = (s: string) => {
  const [y, m, d] = s.split('-').map(Number);
  return new Date(y, m - 1, d);
};

export const startOfDay = (d: Date) => new Date(d.getFullYear(), d.getMonth(), d.getDate());
export const addDays = (d: Date, n: number) => new Date(d.getFullYear(), d.getMonth(), d.getDate() + n);
/** 周一为一周开始 */
export const startOfWeek = (d: Date) => addDays(startOfDay(d), -((d.getDay() + 6) % 7));
/** 1=周一 … 7=周日 */
export const isoWeekday = (d: Date) => d.getDay() || 7;
export const sameDay = (a: Date, b: Date) => ymd(a) === ymd(b);
/** 按日历日计算 b - a 的天数（不受夏令时影响） */
export const dayDiff = (a: Date, b: Date) => Math.round((startOfDay(b).getTime() - startOfDay(a).getTime()) / DAY);

export const toTs = (d: Date) => Math.floor(d.getTime() / 1000);
export const fromTs = (ts: number) => new Date(ts * 1000);
/** unix 秒 <-> <input type="datetime-local"> */
export const toInput = (ts: number | null | undefined) => (ts ? `${ymd(fromTs(ts))}T${hhmm(fromTs(ts))}` : '');
export const fromInput = (s: string) => (s ? toTs(new Date(s)) : null);
export const toMinutes = (hm: string) => {
  const [h, m] = hm.split(':').map(Number);
  return h * 60 + m;
};

/** "今天 18:00" / "明天" / "10月22日 周四" */
export function fmtRelative(ts: number, now: Date, withTime = true): string {
  const d = fromTs(ts);
  const diff = dayDiff(now, d);
  const time = withTime ? ` ${hhmm(d)}` : '';
  if (diff === 0) return `今天${time}`;
  if (diff === 1) return `明天${time}`;
  if (diff === -1) return `昨天${time}`;
  if (diff > 1 && diff < 7) return `周${WEEKDAYS[isoWeekday(d) - 1]}${time}`;
  const year = d.getFullYear() === now.getFullYear() ? '' : `${d.getFullYear()}年`;
  return `${year}${d.getMonth() + 1}月${d.getDate()}日${time}`;
}

export function fmtDate(d: Date): string {
  return `${d.getMonth() + 1}月${d.getDate()}日 周${WEEKDAYS[isoWeekday(d) - 1]}`;
}

/** 毫秒差拆成 天/时/分/秒 */
export function split(ms: number) {
  const past = ms < 0;
  let s = Math.floor(Math.abs(ms) / 1000);
  const days = Math.floor(s / 86400);
  s %= 86400;
  return { past, days, hours: Math.floor(s / 3600), minutes: Math.floor((s % 3600) / 60), seconds: s % 60 };
}

/** 学期第几周：term_start 所在周的周一为第 1 周开始 */
export function termWeek(termStart: string | undefined, d: Date): number | null {
  if (!termStart) return null;
  return Math.floor(dayDiff(startOfWeek(parseYmd(termStart)), d) / 7) + 1;
}

/** "1-8,10,12-16" 是否包含第 week 周 */
export function inWeeks(spec: string, week: number): boolean {
  return spec.split(/[,，]/).some((part) => {
    const [a, b] = part.split('-').map((n) => parseInt(n.trim(), 10));
    if (Number.isNaN(a)) return false;
    return b === undefined ? a === week : !Number.isNaN(b) && week >= a && week <= b;
  });
}

/** 日程是否出现在某天（含跨天与重复规则），规则与 server/src/scheduler.rs 保持一致 */
export function occursOn(ev: CalEvent, day: Date): boolean {
  const s = startOfDay(fromTs(ev.start_at));
  const d = startOfDay(day);
  if (d < s) return false;
  switch (ev.repeat) {
    case 'daily':
      return true;
    case 'weekly':
      return d.getDay() === s.getDay();
    case 'monthly':
      return d.getDate() === s.getDate();
    case 'yearly':
      return d.getMonth() === s.getMonth() && d.getDate() === s.getDate();
    default: {
      const e = ev.end_at ? startOfDay(fromTs(ev.end_at)) : s;
      return d <= e;
    }
  }
}

export const REMIND_OPTIONS: { label: string; value: number | null }[] = [
  { label: '不提醒', value: null },
  { label: '准时', value: 0 },
  { label: '提前 5 分钟', value: 5 },
  { label: '提前 10 分钟', value: 10 },
  { label: '提前 15 分钟', value: 15 },
  { label: '提前 30 分钟', value: 30 },
  { label: '提前 1 小时', value: 60 },
  { label: '提前 2 小时', value: 120 },
  { label: '提前 1 天', value: 1440 },
  { label: '提前 3 天', value: 4320 },
  { label: '提前 1 周', value: 10080 },
];
