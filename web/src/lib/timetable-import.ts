// 课表导入：WakeUp 课程表导出文件、CSV。解析结果先预览，确认后再写入。
import type { Course } from './types';

export type Period = { start: string; end: string };
export type ImportedCourse = Omit<Course, 'id' | 'color' | 'remind_before' | 'channels'>;
export type ImportResult = {
  courses: ImportedCourse[];
  periods?: Period[];
  termStart?: string; // YYYY-MM-DD
  warnings: string[];
};

const pad = (n: number) => String(n).padStart(2, '0');
const normTime = (t: string) => {
  const m = /^(\d{1,2})[:：](\d{2})/.exec(t.trim());
  return m ? `${pad(+m[1])}:${m[2]}` : '';
};

/** 周次列表压缩成 "1-8,10,12-16" */
export function weeksSpec(weeks: number[]): string {
  const ws = [...new Set(weeks)].sort((a, b) => a - b);
  const parts: string[] = [];
  for (let i = 0; i < ws.length; ) {
    let j = i;
    while (j + 1 < ws.length && ws[j + 1] === ws[j] + 1) j++;
    parts.push(i === j ? `${ws[i]}` : `${ws[i]}-${ws[j]}`);
    i = j + 1;
  }
  return parts.join(',');
}

/** 起止周 + 单双周（0 全部，1 单周，2 双周） */
function weekRange(start: number, end: number, type = 0): string {
  const ws: number[] = [];
  for (let w = start; w <= end; w++) if (type === 0 || (type === 1 ? w % 2 === 1 : w % 2 === 0)) ws.push(w);
  return weeksSpec(ws);
}

// ---------- WakeUp 课程表 ----------
// 导出文件是若干行 JSON：表信息、作息时间、表设置（含开学日期）、课程定义、课程安排。按内容特征识别，不依赖行序。

export function parseWakeUp(text: string): ImportResult {
  const blocks = text
    .split(/\r?\n/)
    .map((l) => l.trim())
    .filter(Boolean)
    .map((l) => {
      try {
        return JSON.parse(l);
      } catch {
        return null;
      }
    })
    .filter((x) => x != null);
  const arr = (pred: (o: any) => boolean): any[] =>
    blocks.find((b) => Array.isArray(b) && b.length && pred(b[0])) ?? [];

  const nodes = arr((o) => 'node' in o && 'startTime' in o);
  const defs = arr((o) => 'courseName' in o);
  const details = arr((o) => 'startNode' in o && 'day' in o);
  const setting = blocks.find((b) => !Array.isArray(b) && typeof b === 'object' && 'startDate' in b);
  if (!defs.length || !details.length) throw new Error('没有找到课程数据，请确认是 WakeUp 课程表导出的文件');

  const periods: Period[] = nodes
    .toSorted((a, b) => a.node - b.node)
    .map((n) => ({ start: normTime(n.startTime), end: normTime(n.endTime) }));
  const nodeTime = (n: number) => periods[n - 1];
  const nameOf = new Map(defs.map((d) => [d.id, String(d.courseName)]));
  const warnings: string[] = [];

  const courses: ImportedCourse[] = [];
  for (const d of details) {
    const name = nameOf.get(d.id);
    const from = Number(d.startNode);
    const to = from + Math.max(1, Number(d.step) || 1) - 1;
    const start = d.ownTime && d.startTime ? normTime(d.startTime) : nodeTime(from)?.start;
    const end = d.ownTime && d.endTime ? normTime(d.endTime) : nodeTime(to)?.end;
    if (!name || !start || !end) {
      warnings.push(`跳过：${name ?? `课程 ${d.id}`}（缺少时间）`);
      continue;
    }
    courses.push({
      name,
      teacher: d.teacher || null,
      room: d.room || null,
      weekday: Number(d.day),
      start_time: start,
      end_time: end,
      weeks: weekRange(Number(d.startWeek) || 1, Number(d.endWeek) || 20, Number(d.type) || 0),
      period_from: d.ownTime ? null : from,
      period_to: d.ownTime ? null : to,
    });
  }

  let termStart: string | undefined;
  const m = /^(\d{4})-(\d{1,2})-(\d{1,2})/.exec(String(setting?.startDate ?? ''));
  if (m) termStart = `${m[1]}-${pad(+m[2])}-${pad(+m[3])}`;
  return { courses, periods: periods.length ? periods : undefined, termStart, warnings };
}

// ---------- CSV ----------

/** 支持引号和逗号转义的简易 CSV 解析 */
export function parseCsvRows(text: string): string[][] {
  const rows: string[][] = [];
  let row: string[] = [];
  let cell = '';
  let quoted = false;
  const s = text.replace(/^﻿/, '');
  for (let i = 0; i < s.length; i++) {
    const c = s[i];
    if (quoted) {
      if (c === '"' && s[i + 1] === '"') (cell += '"'), i++;
      else if (c === '"') quoted = false;
      else cell += c;
    } else if (c === '"') quoted = true;
    else if (c === ',' || c === '\t') row.push(cell), (cell = '');
    else if (c === '\n' || c === '\r') {
      if (c === '\r' && s[i + 1] === '\n') i++;
      row.push(cell), rows.push(row), (row = []), (cell = '');
    } else cell += c;
  }
  if (cell || row.length) row.push(cell), rows.push(row);
  return rows.filter((r) => r.some((c) => c.trim()));
}

const COLUMNS: Record<string, string[]> = {
  name: ['课程名', '课程', '课程名称', 'name'],
  weekday: ['星期', '周几', 'weekday', 'day'],
  start: ['开始时间', '开始', 'start', 'start_time'],
  end: ['结束时间', '结束', 'end', 'end_time'],
  periods: ['节次', '节', 'periods'],
  weeks: ['周次', '上课周', 'weeks'],
  room: ['教室', '地点', 'room', 'location'],
  teacher: ['老师', '教师', 'teacher'],
};

const CN_DAYS = '一二三四五六日天';
function parseWeekday(v: string): number {
  const t = v.trim().replace(/^(周|星期|礼拜)/, '');
  const n = Number(t);
  if (n >= 1 && n <= 7) return n;
  const i = CN_DAYS.indexOf(t);
  return i < 0 ? 0 : Math.min(i + 1, 7);
}

/** CSV：表头按名称识别；时间可直接给，也可给节次（需先设置作息时间） */
export function parseCsv(text: string, periods: Period[]): ImportResult {
  const [head, ...rows] = parseCsvRows(text);
  if (!head) throw new Error('文件是空的');
  const idx: Record<string, number> = {};
  head.forEach((h, i) => {
    const key = Object.entries(COLUMNS).find(([, names]) => names.includes(h.trim().toLowerCase()))?.[0];
    if (key) idx[key] = i;
  });
  if (idx.name == null || idx.weekday == null) throw new Error('缺少「课程名」或「星期」列');
  if (idx.start == null && idx.periods == null) throw new Error('需要「开始时间/结束时间」或「节次」列');

  const courses: ImportedCourse[] = [];
  const warnings: string[] = [];
  rows.forEach((r, n) => {
    const get = (k: string) => (idx[k] == null ? '' : (r[idx[k]] ?? '').trim());
    const name = get('name');
    const weekday = parseWeekday(get('weekday'));
    let start = normTime(get('start'));
    let end = normTime(get('end'));
    let pf: number | null = null;
    let pt: number | null = null;
    const pm = /^(\d+)(?:\s*[-~至]\s*(\d+))?/.exec(get('periods'));
    if ((!start || !end) && pm) {
      pf = +pm[1];
      pt = +(pm[2] ?? pm[1]);
      start = periods[pf - 1]?.start ?? '';
      end = periods[pt - 1]?.end ?? '';
    }
    if (!name || !weekday || !start || !end) {
      warnings.push(`第 ${n + 2} 行：信息不完整，已跳过`);
      return;
    }
    courses.push({
      name,
      weekday,
      start_time: start,
      end_time: end,
      weeks: get('weeks').replace(/周/g, '').replace(/，/g, ',') || '1-16',
      room: get('room') || null,
      teacher: get('teacher') || null,
      period_from: pf,
      period_to: pt,
    });
  });
  return { courses, warnings };
}

export const CSV_TEMPLATE =
  '课程名,星期,开始时间,结束时间,节次,周次,教室,老师\n高等数学,周一,08:00,09:40,,1-16,A101,王老师\n大学英语,周三,,,3-4,"1-8,10-16",B203,李老师\n';

/** 常见的大学作息（每节 45 分钟） */
export const DEFAULT_PERIODS: Period[] = [
  ['08:00', '08:45'],
  ['08:55', '09:40'],
  ['10:00', '10:45'],
  ['10:55', '11:40'],
  ['14:00', '14:45'],
  ['14:55', '15:40'],
  ['16:00', '16:45'],
  ['16:55', '17:40'],
  ['19:00', '19:45'],
  ['19:55', '20:40'],
  ['20:50', '21:35'],
].map(([start, end]) => ({ start, end }));

/** 设置里保存的作息时间（JSON） */
export function parsePeriods(s: string | undefined): Period[] {
  try {
    const v = s ? JSON.parse(s) : [];
    return Array.isArray(v) ? v : [];
  } catch {
    return [];
  }
}
