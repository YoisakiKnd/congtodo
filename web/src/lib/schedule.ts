// Schedule-X 适配：把业务数据（日程 / 课程 / 待办）展开成日历事件
import type { CalendarEvent, CalendarType } from '@schedule-x/calendar';
import { PALETTE } from './store.svelte';
import { addDays, fromTs, inWeeks, isoWeekday, occursOn, startOfDay, termWeek, toMinutes, ymd } from './time';
import type { CalEvent, Course, Todo } from './types';

export const TZ = Temporal.Now.timeZoneId();

export type Range = { start: Date; end: Date };
export type EventRef = { kind: 'event' | 'course' | 'todo'; id: number };

// ---------- 颜色：每个调色板颜色对应一个 Schedule-X calendar ----------

const rgb = (hex: string) => [1, 3, 5].map((i) => parseInt(hex.slice(i, i + 2), 16));
const mix = (a: string, b: string, w: number) =>
  '#' +
  rgb(a)
    .map((v, i) => Math.round(v * (1 - w) + rgb(b)[i] * w))
    .map((v) => v.toString(16).padStart(2, '0'))
    .join('');

const calendarType = (name: string, c: string): CalendarType => ({
  colorName: name,
  lightColors: { main: c, container: mix(c, '#ffffff', 0.82), onContainer: mix(c, '#000000', 0.55) },
  darkColors: { main: c, container: mix(c, '#1c1917', 0.65), onContainer: mix(c, '#ffffff', 0.75) },
});

export const CALENDARS: Record<string, CalendarType> = {
  ...Object.fromEntries(PALETTE.map((c, i) => [`c${i}`, calendarType(`c${i}`, c)])),
  todo: calendarType('todo', '#f5a524'),
};

export const calendarId = (color: string | null) => `c${Math.max(0, PALETTE.indexOf(color ?? ''))}`;

// ---------- 时间转换 ----------

export const zdt = (d: Date) => Temporal.Instant.fromEpochMilliseconds(d.getTime()).toZonedDateTimeISO(TZ);
export const plainDate = (d: Date) => Temporal.PlainDate.from(ymd(d));
export const toDate = (t: Temporal.ZonedDateTime | Temporal.PlainDate) =>
  t instanceof Temporal.PlainDate ? new Date(t.year, t.month - 1, t.day) : new Date(t.epochMilliseconds);
export const toRange = (r: { start: Temporal.ZonedDateTime; end: Temporal.ZonedDateTime }): Range => ({
  start: toDate(r.start),
  end: toDate(r.end),
});

const at = (d: Date, hm: string) => {
  const m = toMinutes(hm);
  return new Date(d.getFullYear(), d.getMonth(), d.getDate(), Math.floor(m / 60), m % 60);
};

function* days(r: Range) {
  for (let d = startOfDay(r.start); d <= r.end; d = addDays(d, 1)) yield d;
}

// ---------- 展开 ----------

export function eventInstances(events: CalEvent[], r: Range): CalendarEvent[] {
  const out: CalendarEvent[] = [];
  for (const e of events) {
    const s = fromTs(e.start_at);
    const dur = Math.max(0, e.end_at ? (e.end_at - e.start_at) * 1000 : e.all_day ? 0 : 3600_000);
    const push = (start: Date, key: string) => {
      let end = new Date(start.getTime() + dur);
      // 没有结束时间的日程默认 1 小时，但不跨到第二天
      if (!e.end_at && !e.all_day) end = new Date(Math.min(end.getTime(), at(start, '23:59').getTime()));
      out.push({
        id: `e${e.id}_${key}`,
        title: e.title,
        description: e.note ?? undefined,
        calendarId: calendarId(e.color),
        start: e.all_day ? plainDate(start) : zdt(start),
        end: e.all_day ? plainDate(end) : zdt(end),
        ref: { kind: 'event', id: e.id } satisfies EventRef,
      });
    };
    if (e.repeat === 'none') {
      if (s.getTime() + dur >= r.start.getTime() && s <= r.end) push(s, 'x');
    } else {
      for (const d of days(r)) if (occursOn(e, d)) push(at(d, `${s.getHours()}:${s.getMinutes()}`), ymd(d));
    }
  }
  return out;
}

export function courseInstances(courses: Course[], r: Range, termStart: string | undefined, showInactive: boolean): CalendarEvent[] {
  const out: CalendarEvent[] = [];
  for (const d of days(r)) {
    const w = termWeek(termStart, d);
    for (const c of courses) {
      if (c.weekday !== isoWeekday(d)) continue;
      const active = w == null || inWeeks(c.weeks, w);
      if (!active && !showInactive) continue;
      out.push({
        id: `c${c.id}_${ymd(d)}`,
        title: c.name,
        location: c.room || undefined,
        people: c.teacher ? [c.teacher] : undefined,
        calendarId: calendarId(c.color),
        start: zdt(at(d, c.start_time)),
        end: zdt(at(d, c.end_time)),
        ref: { kind: 'course', id: c.id } satisfies EventRef,
        _options: active ? undefined : { additionalClasses: ['is-inactive'] },
      });
    }
  }
  return out;
}

export function todoInstances(todos: Todo[], r: Range): CalendarEvent[] {
  return todos
    .filter((t) => !t.done && t.due_at && fromTs(t.due_at) >= startOfDay(r.start) && fromTs(t.due_at) <= r.end)
    .map((t) => ({
      id: `t${t.id}`,
      title: `☐ ${t.title}`,
      calendarId: 'todo',
      start: plainDate(fromTs(t.due_at!)),
      end: plainDate(fromTs(t.due_at!)),
      ref: { kind: 'todo', id: t.id } satisfies EventRef,
    }));
}
