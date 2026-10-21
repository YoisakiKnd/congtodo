// 日历格子的自定义渲染：日期 + 农历 / 节日 / 节气 + 法定假日「休 / 班」角标。
// Schedule-X 只在日期变化时调用渲染函数，所以这里记住渲染过的格子，节假日数据到了再统一重绘。
import type { CustomComponentFn } from '@schedule-x/calendar';
import { dayLabel } from './lunar';
import { holidays } from './store.svelte';
import { parseYmd, WEEKDAYS, isoWeekday, ymd } from './time';

type Cell = { kind: 'month' | 'week'; date: Date };

export function createCellRenderer() {
  const cells = new Map<HTMLElement, Cell>();

  function paint(el: HTMLElement, { kind, date }: Cell) {
    const key = ymd(date);
    const today = key === ymd(new Date());
    const label = dayLabel(date);
    const h = holidays.get(key);
    const badge = h ? `<span class="cx-badge ${h.off ? 'is-off' : 'is-work'}" title="${h.name}${h.off ? '放假' : '调休上班'}">${h.off ? '休' : '班'}</span>` : '';
    const sub = `<span class="cx-sub${label.festival ? ' is-fest' : ''}">${h?.off && !label.festival ? h.name : label.text}</span>`;
    el.innerHTML =
      kind === 'month'
        ? `<div class="cx-date"><span class="sx__month-grid-day__header-date${today ? ' sx__is-today' : ''}">${date.getDate()}</span>${sub}${badge}</div>`
        : `<div class="sx__week-grid__day-name">周${WEEKDAYS[isoWeekday(date) - 1]}</div><div class="sx__week-grid__date-number">${date.getDate()}</div><div class="cx-week-sub">${sub}${badge}</div>`;
  }

  const monthGridDate: CustomComponentFn = (el, props) => {
    const cell: Cell = { kind: 'month', date: props.jsDate as Date };
    cells.set(el, cell);
    paint(el, cell);
  };
  const weekGridDate: CustomComponentFn = (el, props) => {
    const cell: Cell = { kind: 'week', date: parseYmd(props.date as string) };
    cells.set(el, cell);
    paint(el, cell);
  };

  /** 节假日数据更新后重绘仍在页面上的格子 */
  function repaint() {
    for (const [el, cell] of cells) {
      if (el.isConnected) paint(el, cell);
      else cells.delete(el);
    }
  }

  return { custom: { monthGridDate, weekGridDate }, repaint };
}
