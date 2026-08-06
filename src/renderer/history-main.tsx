import React from 'react';
import ReactDOM from 'react-dom/client';
import HistoryWindow from '@/renderer/windows/history-window';
import '@/renderer/styles/app.css';

ReactDOM.createRoot(document.getElementById('root')!).render(
  <React.StrictMode>
    <HistoryWindow />
  </React.StrictMode>
);
