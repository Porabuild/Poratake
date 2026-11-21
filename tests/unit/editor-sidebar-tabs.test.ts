import { beforeEach, describe, expect, it, vi } from 'vitest';

const { exportPanelLoaded, exporterLoaded, wallpaperPanelLoaded } = vi.hoisted(
  () => ({
    exportPanelLoaded: vi.fn(),
    exporterLoaded: vi.fn(),
    wallpaperPanelLoaded: vi.fn(),
  })
);

vi.mock('@/renderer/components/video-editor/export-settings-panel', () => {
  exportPanelLoaded();
  return { default: () => null };
});

vi.mock('@/renderer/components/video-editor/export', () => {
  exporterLoaded();
  return { WebCodecsExporter: class {} };
});

vi.mock('@/renderer/components/editor/wallpaper', () => {
  wallpaperPanelLoaded();
  return { default: () => null };
});

describe('EditorSidebarTabs', () => {
  vi.setConfig({ testTimeout: 30_000 });

  beforeEach(() => {
    vi.clearAllMocks();
  });

  it('names the sidebar buttons and keeps Zoom enabled before a segment exists', async () => {
    const React = await import('react');
    vi.stubGlobal('React', React);
    const { renderToStaticMarkup } = await import('react-dom/server');
    const { default: EditorSidebarTabs } =
      await import('@/renderer/components/video-editor/editor-sidebar-tabs');

    const markup = renderToStaticMarkup(
      React.createElement(EditorSidebarTabs, {
        activeTab: 'cursor',
        onTabChange: vi.fn(),
      })
    );

    const labels = [
      ...markup.matchAll(/<button[^>]*aria-label="([^"]+)"/g),
    ].map(match => match[1]);
    expect(labels).toEqual([
      'Cursor',
      'Zoom',
      'Drawing',
      'Camera',
      'Audio',
      'Wallpaper',
      'Keyboard',
      'Subtitles',
      'First Frame',
      'Export',
    ]);
    expect(markup).not.toMatch(/<button[^>]*disabled(?:=|>)/);
  });

  it('loads a sidebar panel only after intent to open it', async () => {
    const { preloadEditorSidebarTab } =
      await import('@/renderer/components/video-editor/editor-sidebar-panel-loaders');

    expect(exportPanelLoaded).not.toHaveBeenCalled();

    preloadEditorSidebarTab('export');

    await vi.waitFor(() => expect(exportPanelLoaded).toHaveBeenCalledOnce());
  });

  it('does not load the export pipeline with the export hook', async () => {
    await import('@/renderer/components/video-editor/hooks/use-video-export');

    expect(exporterLoaded).not.toHaveBeenCalled();
  });

  it('does not load wallpaper controls with the screenshot editor', async () => {
    await import('@/renderer/windows/screenshot-window');

    expect(wallpaperPanelLoaded).not.toHaveBeenCalled();
  });
});
