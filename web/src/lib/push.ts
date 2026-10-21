// 浏览器原生推送：订阅后作为一个 webpush 渠道保存，服务端用 VAPID 推送到这个浏览器
import { api } from './api';

export const pushSupported = () => 'serviceWorker' in navigator && 'PushManager' in window && 'Notification' in window;

/** iOS 只有「添加到主屏幕」后的网页应用才支持推送 */
export const isIosBrowser = () =>
  /iphone|ipad|ipod/i.test(navigator.userAgent) && !(window.matchMedia('(display-mode: standalone)').matches);

function urlB64ToBytes(b64: string) {
  const s = atob((b64 + '='.repeat((4 - (b64.length % 4)) % 4)).replace(/-/g, '+').replace(/_/g, '/'));
  return Uint8Array.from(s, (c) => c.charCodeAt(0));
}

export function deviceName() {
  const ua = navigator.userAgent;
  const os = /iphone|ipad/i.test(ua) ? 'iOS' : /android/i.test(ua) ? 'Android' : /mac/i.test(ua) ? 'macOS' : /windows/i.test(ua) ? 'Windows' : 'Linux';
  const browser = /edg\//i.test(ua) ? 'Edge' : /firefox/i.test(ua) ? 'Firefox' : /chrome/i.test(ua) ? 'Chrome' : /safari/i.test(ua) ? 'Safari' : '浏览器';
  return `${browser} · ${os}`;
}

export async function subscribePush(): Promise<{ endpoint: string; p256dh: string; auth: string }> {
  if (!pushSupported()) throw new Error('当前浏览器不支持推送');
  if ((await Notification.requestPermission()) !== 'granted') throw new Error('没有获得通知权限，请在浏览器设置中允许');
  const reg = await navigator.serviceWorker.ready;
  const { key } = await api.get<{ key: string }>('/push/key');
  const sub =
    (await reg.pushManager.getSubscription()) ??
    (await reg.pushManager.subscribe({ userVisibleOnly: true, applicationServerKey: urlB64ToBytes(key) }));
  const json = sub.toJSON();
  return { endpoint: json.endpoint!, p256dh: json.keys!.p256dh, auth: json.keys!.auth };
}

/** 当前浏览器已有的订阅地址（用于判断「这台设备」是否已启用） */
export async function currentEndpoint(): Promise<string | null> {
  if (!pushSupported()) return null;
  const reg = await navigator.serviceWorker.getRegistration();
  return (await reg?.pushManager.getSubscription())?.endpoint ?? null;
}
