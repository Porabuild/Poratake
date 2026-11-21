const { app, screen } = require('electron');
app.whenReady().then(() => {
  const primary = screen.getPrimaryDisplay();
  const displays = screen.getAllDisplays().map(display => ({
    id: display.id,
    bounds: display.bounds,
    workArea: display.workArea,
    scaleFactor: display.scaleFactor,
    primary: display.id === primary.id,
  }));
  console.log(JSON.stringify(displays));
  app.quit();
});
