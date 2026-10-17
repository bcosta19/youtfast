// Minimal "YouTube desktop app": a window that renders youtube.com, as wrapper apps do.
const { app, BrowserWindow } = require('electron');
app.whenReady().then(() => {
  const win = new BrowserWindow({ width: 1920, height: 1080, autoHideMenuBar: true });
  win.loadURL(process.env.BENCH_URL || 'https://www.youtube.com/');
});
app.on('window-all-closed', () => app.quit());
