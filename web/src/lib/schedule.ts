// Schedule-X 适配：把业务数据（日程 / 课程 / 待办）展开成日历事件
import type { CalendarEvent, CalendarType } from '@schedule-x/calendar';
import { PALETTE } from './store.svelte';
import { addDays, fromTs, inWeeks, isoWeekday, occursOn, startOfDay, termWeek, toMinutes, ymd } from './time';
import type { CalEvent, Countdown, Course, FeedEvent, Todo } from './types';

export const TZ = Temporal.Now.timeZoneId();

export type Range = { start: Date; end: Date };
export type EventRef = { kind: 'event' | 'course' | 'todo' | 'feed' | 'countdown'; id: number };

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

/** offDay：法定假日当天返回 true，课程会淡化显示（默认不提醒） */
export function courseInstances(
  courses: Course[],
  r: Range,
  termStart: string | undefined,
  showInactive: boolean,
  offDay: (d: Date) => boolean = () => false,
): CalendarEvent[] {
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
        _options: active && !offDay(d) ? undefined : { additionalClasses: ['is-inactive'] },
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

/** 订阅日历（只读）。全天事件的 end_at 存的是最后一天的 0 点 */
export function feedInstances(events: FeedEvent[]): CalendarEvent[] {
  return events.map((e, i) => {
    const start = fromTs(e.start_at);
    const end = e.end_at ? fromTs(e.end_at) : e.all_day ? start : new Date(start.getTime() + 3600_000);
    return {
      id: `f${e.feed_id}_${i}`,
      title: e.title,
      location: e.location || undefined,
      description: `来自订阅：${e.feed}`,
      calendarId: calendarId(e.color),
      start: e.all_day ? plainDate(start) : zdt(start),
      end: e.all_day ? plainDate(end) : zdt(end < start ? start : end),
      ref: { kind: 'feed', id: e.feed_id } satisfies EventRef,
      _options: { additionalClasses: ['is-feed'] },
    };
  });
}

/** 倒数日（全天）；每年重复的按可见范围内的每一年展开，标题带周年 */
export function countdownInstances(list: Countdown[], r: Range): CalendarEvent[] {
  const out: CalendarEvent[] = [];
  for (const c of list) {
    const t = fromTs(c.target_at);
    const dates: Date[] = [];
    if (c.repeat === 'yearly') {
      for (let y = Math.max(r.start.getFullYear(), t.getFullYear()); y <= r.end.getFullYear(); y++) {
        const d = new Date(y, t.getMonth(), t.getDate());
        if (d.getMonth() === t.getMonth() && d >= startOfDay(r.start) && d <= r.end) dates.push(d);
      }
    } else if (t >= startOfDay(r.start) && t <= r.end) dates.push(t);
    for (const d of dates) {
      const years = d.getFullYear() - t.getFullYear();
      out.push({
        id: `cd${c.id}_${ymd(d)}`,
        title: `⏳ ${c.title}${c.repeat === 'yearly' && years > 0 ? `（${years} 周年）` : ''}`,
        calendarId: calendarId(c.color),
        start: plainDate(d),
        end: plainDate(d),
        ref: { kind: 'countdown', id: c.id } satisfies EventRef,
      });
    }
  }
  return out;
}
