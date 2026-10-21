// 与 server/src/db.rs 的表结构一一对应；时间戳均为 unix 秒

/** 条目指定的推送渠道 id；null = 按渠道的类型设置 */
export type ChannelIds = number[] | null;

export interface Countdown {
  id: number;
  title: string;
  target_at: number;
  color: string | null;
  pinned: number;
  remind_before: number | null;
  repeat: 'none' | 'yearly';
  image_id: number | null;
  channels: ChannelIds;
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
  channels: ChannelIds;
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
  period_from: number | null; // 按节次录入时的起止节
  period_to: number | null;
  channels: ChannelIds;
}

export type TodoRepeat = 'none' | 'daily' | 'weekdays' | 'weekly' | 'monthly' | 'yearly';

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
  user_id: number; // 创建者（共享清单里可能是别人）
  list_id: number | null;
  parent_id: number | null;
  repeat: TodoRepeat;
  channels: ChannelIds;
}

export interface TodoList {
  id: number;
  name: string;
  color: string | null;
  sort: number;
  owner_id: number;
  owner: string;
  shared: boolean; // 别人分享给我的
  members: { id: number; username: string }[];
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
  days: string | null; // "1,2,3,4,5"，null = 每天
  goal: number; // 每天需完成的次数
  unit: string | null;
  channels: ChannelIds;
}

export interface HabitLog {
  count: number;
  note: string | null;
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

export type ChannelKind = 'webhook' | 'ntfy' | 'bark' | 'webpush';

/** 提醒类型（渠道可按类型过滤） */
export type NotifyKind = 'todo' | 'event' | 'course' | 'habit' | 'countdown' | 'digest' | 'weekly';

export interface Channel {
  id: number;
  name: string;
  kind: ChannelKind;
  config: Record<string, any>;
  enabled: number;
  kinds: NotifyKind[] | null; // null = 接收全部类型
}

export interface Feed {
  id: number;
  name: string;
  url: string;
  color: string | null;
  enabled: number;
  last_sync: number | null;
  last_error: string | null;
}

export interface FeedEvent {
  feed_id: number;
  uid: string;
  title: string;
  start_at: number;
  end_at: number | null;
  all_day: number;
  location: string | null;
  color: string | null;
  feed: string;
}

export interface FocusSession {
  id: number;
  todo_id: number | null;
  started_at: number;
  minutes: number;
  note: string | null;
}

export interface OutboxItem {
  id: number;
  kind: string;
  title: string;
  body: string;
  status: 'pending' | 'sent' | 'failed';
  attempts: number;
  next_at: number;
  error: string | null;
  created_at: number;
  sent_at: number | null;
  channel: string | null;
}

export interface Holiday {
  name: string;
  off: boolean; // true 放假，false 调休上班
}
