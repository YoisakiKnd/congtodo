// 农历与节气：农历用浏览器内置的 Intl 中国历法，节气用通用公式计算（2000-2099 年，含已知例外）

const fmt = new Intl.DateTimeFormat('zh-CN-u-ca-chinese', { month: 'long', day: 'numeric' });

const DAY_NAMES = ['初', '十', '廿', '三'];
const DIGITS = ['十', '一', '二', '三', '四', '五', '六', '七', '八', '九'];

/** 1 → 初一，10 → 初十，15 → 十五，20 → 二十，21 → 廿一，30 → 三十 */
export function lunarDayName(n: number): string {
  if (n === 10) return '初十';
  if (n === 20) return '二十';
  if (n === 30) return '三十';
  return DAY_NAMES[Math.floor(n / 10)] + DIGITS[n % 10];
}

export function lunar(d: Date): { month: string; day: number; dayName: string } {
  const parts = fmt.formatToParts(new Date(d.getFullYear(), d.getMonth(), d.getDate(), 12));
  const month = parts.find((p) => p.type === 'month')?.value ?? '';
  const day = Number(parts.find((p) => p.type === 'day')?.value ?? 0);
  return { month, day, dayName: lunarDayName(day) };
}

// 21 世纪 C 值，顺序：小寒 大寒 立春 雨水 … 冬至（每月两个，从 1 月开始）
const TERMS = ['小寒', '大寒', '立春', '雨水', '惊蛰', '春分', '清明', '谷雨', '立夏', '小满', '芒种', '夏至',
  '小暑', '大暑', '立秋', '处暑', '白露', '秋分', '寒露', '霜降', '立冬', '小雪', '大雪', '冬至'];
const C = [5.4055, 20.12, 3.87, 18.73, 5.63, 20.646, 4.81, 20.1, 5.52, 21.04, 5.678, 21.37,
  7.108, 22.83, 7.5, 23.13, 7.646, 23.042, 8.318, 23.438, 7.438, 22.36, 7.18, 21.94];
// 公式的已知例外：[年份, 节气序号, 修正天数]
const EXCEPTIONS: [number, number, number][] = [
  [2019, 0, -1], [2082, 1, 1], [2026, 3, -1], [2084, 5, 1], [2008, 9, 1],
  [2016, 12, 1], [2002, 14, 1], [2089, 19, 1], [2089, 20, 1], [2021, 23, -1],
];

/** 某年第 i 个节气（0 = 小寒）在几号 */
function termDay(year: number, i: number): number {
  const y = year % 100;
  // 1-2 月的节气用上一年的闰年数
  const leaps = i < 4 ? Math.floor((y - 1) / 4) : Math.floor(y / 4);
  const fix = EXCEPTIONS.find(([yy, ii]) => yy === year && ii === i)?.[2] ?? 0;
  return Math.floor(y * 0.2422 + C[i]) - leaps + fix;
}

/** 当天是节气则返回名称 */
export function solarTerm(d: Date): string | null {
  const year = d.getFullYear();
  if (year < 2000 || year > 2099) return null;
  const i = d.getMonth() * 2 + (d.getDate() >= 15 ? 1 : 0);
  return termDay(year, i) === d.getDate() ? TERMS[i] : null;
}

// 农历节日（按农历月日）
const LUNAR_FESTIVALS: Record<string, string> = {
  '正月-1': '春节',
  '正月-15': '元宵',
  '五月-5': '端午',
  '七月-7': '七夕',
  '八月-15': '中秋',
  '九月-9': '重阳',
  '腊月-8': '腊八',
};
// 公历节日
const SOLAR_FESTIVALS: Record<string, string> = {
  '1-1': '元旦',
  '2-14': '情人节',
  '3-8': '妇女节',
  '5-1': '劳动节',
  '5-4': '青年节',
  '6-1': '儿童节',
  '9-10': '教师节',
  '10-1': '国庆节',
  '12-25': '圣诞节',
};

/** 日历格子里显示的副标题：节日 > 节气 > 农历（初一显示月份） */
export function dayLabel(d: Date): { text: string; festival: boolean } {
  const l = lunar(d);
  const festival =
    LUNAR_FESTIVALS[`${l.month}-${l.day}`] ??
    SOLAR_FESTIVALS[`${d.getMonth() + 1}-${d.getDate()}`] ??
    solarTerm(d);
  if (festival) return { text: festival, festival: true };
  // 除夕：腊月最后一天（下一天是正月初一）
  const next = lunar(new Date(d.getFullYear(), d.getMonth(), d.getDate() + 1));
  if (next.month === '正月' && next.day === 1) return { text: '除夕', festival: true };
  return { text: l.day === 1 ? l.month : l.dayName, festival: false };
}
