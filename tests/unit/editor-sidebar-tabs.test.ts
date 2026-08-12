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
  beforeEach(() => {
    vi.clearAllMocks();
  });

  it('keeps the Zoom tab enabled before a zoom segment exists', async () => {
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
