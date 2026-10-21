// 与 server/src/db.rs 的表结构一一对应；时间戳均为 unix 秒

export interface Countdown {
  id: number;
  title: string;
  target_at: number;
  color: string | null;
  pinned: number;
  remind_before: number | null;
}

export type Repeat = 'none' | 'daily' | 'weekly' | 'monthly' | 'yearly';

export interface CalEvent {
  id: number;
  title: string;
  start_at: number;
  end_at: number | null;
  all_day: number;
  repeat: Repeat;
  color: string | null;
  note: string | null;
  remind_before: number | null;
}

export interface Course {
  id: number;
  name: string;
  teacher: string | null;
  room: string | null;
  weekday: number; // 1=周一 … 7=周日
  start_time: string; // "08:00"
  end_time: string;
  weeks: string; // "1-16"
  color: string | null;
  remind_before: number | null;
}

export interface Todo {
  id: number;
  title: string;
  note: string | null;
  due_at: number | null;
  remind_at: number | null;
  priority: number; // 0 普通 1 重要 2 紧急
  done: number;
  done_at: number | null;
  created_at: number;
}

export interface Habit {
  id: number;
  name: string;
  icon: string | null;
  color: string | null;
  target: number; // 每周目标次数
  remind_time: string | null;
  archived: number;
  created_at: number;
}

/** 管理员视角的用户信息（/api/admin/users） */
export interface AdminUser {
  id: number;
  username: string;
  is_admin: number;
  disabled: number;
  created_at: number;
  last_login_at: number | null;
  sessions: number; // 有效会话数
  items: number; // 待办 + 日程 + 课程 + 习惯 + 倒数日 条数
}

export type SignupMode = 'first' | 'open' | 'closed';

export type ChannelKind = 'webhook' | 'ntfy' | 'bark';

export interface Channel {
  id: number;
  name: string;
  kind: ChannelKind;
  config: Record<string, any>;
  enabled: number;
}
