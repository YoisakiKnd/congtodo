// 全局的编辑器 / 面板状态：在任何页面都能直接打开，不需要跳转。
// 对应的组件挂在 App.svelte 里，监听这里的状态。
import type { CalEvent, Course } from './types';

export type EventTarget = { event?: CalEvent; start?: Date; allDay?: boolean };
export type CourseTarget = { course?: Course; weekday?: number; start?: string; end?: string };
export type PanelName = 'habits' | 'countdowns' | 'focus';

export const ui = $state({
  event: null as EventTarget | null,
  course: null as CourseTarget | null,
  periods: false,
  importCourses: false,
  panel: null as PanelName | null,
});

export const openEvent = (t: EventTarget = {}) => (ui.event = t);
export const openCourse = (t: CourseTarget = {}) => (ui.course = t);
export const openPeriods = () => (ui.periods = true);
export const openImport = () => (ui.importCourses = true);
export const openPanel = (p: PanelName) => (ui.panel = p);
