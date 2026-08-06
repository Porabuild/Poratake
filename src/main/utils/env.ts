import { app } from 'electron';

export const isProduction = app.isPackaged;

export const isDev = !isProduction;

export const devServerUrl = process.env.VITE_DEV_SERVER_URL;

export const getAppVersion = (): string => {
  if (isDev && process.env.CAPTY_DEV_APP_VERSION) {
    return process.env.CAPTY_DEV_APP_VERSION;
  }
  return app.getVersion();
};
