import { Notification } from 'electron';

const TRANSIENT_NOTIFICATION_DURATION_MS = 5_000;

export function showNotification(title: string, body: string): void {
  const notification = new Notification({
    title,
    body,
  });
  notification.show();
}

export function showTransientNotification(title: string, body: string): void {
  const notification = new Notification({
    title,
    body,
    silent: true,
    timeoutType: 'default',
  });
  const timeout = setTimeout(
    () => notification.close(),
    TRANSIENT_NOTIFICATION_DURATION_MS
  );
  timeout.unref?.();

  notification.once('click', () => notification.close());
  notification.once('close', () => {
    clearTimeout(timeout);
    notification.close();
  });
  notification.show();
}
