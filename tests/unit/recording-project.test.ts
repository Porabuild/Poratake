import { describe, it, expect, vi, beforeEach } from 'vitest';
import path from 'path';

vi.mock('fs', () => ({
  existsSync: vi.fn(),
  mkdirSync: vi.fn(),
  rmSync: vi.fn(),
  renameSync: vi.fn(),
  unlinkSync: vi.fn(),
}));

vi.mock('fs/promises', () => ({
  default: {
    readdir: vi.fn(),
    unlink: vi.fn(),
  },
}));

describe('recording-project', () => {
  beforeEach(() => {
    vi.clearAllMocks();
    vi.resetModules();
  });

  describe('isRecordingProject', () => {
    it('returns true when path ends with .poratake', async () => {
      const { isRecordingProject } =
        await import('@/main/capture/video/recording-project');
      expect(isRecordingProject('/path/to/My Recording.poratake')).toBe(true);
    });

    it('returns false when path does not end with .poratake', async () => {
      const { isRecordingProject } =
        await import('@/main/capture/video/recording-project');
      expect(isRecordingProject('/path/to/video.mov')).toBe(false);
      expect(isRecordingProject('/path/to/video.mp4')).toBe(false);
    });
  });

  describe('getProjectFolder', () => {
    it('returns same path when input is a project folder', async () => {
      const { getProjectFolder } =
        await import('@/main/capture/video/recording-project');
      expect(getProjectFolder('/path/to/Rec.poratake')).toBe(
        '/path/to/Rec.poratake'
      );
    });

    it('returns parent dir when input is a file inside a project', async () => {
      const { getProjectFolder } =
        await import('@/main/capture/video/recording-project');
      expect(getProjectFolder('/path/to/Rec.poratake/recording.mov')).toBe(
        '/path/to/Rec.poratake'
      );
    });

    it('returns null when not in a project', async () => {
      const { getProjectFolder } =
        await import('@/main/capture/video/recording-project');
      expect(getProjectFolder('/path/to/video.mov')).toBeNull();
    });
  });

  describe('path getters', () => {
    it('getRecordingVideoPath returns recording.mov inside project', async () => {
      const { getRecordingVideoPath } =
        await import('@/main/capture/video/recording-project');
      expect(getRecordingVideoPath('/path/to/Rec.poratake')).toBe(
        path.join('/path/to/Rec.poratake', 'recording.mov')
      );
      expect(getRecordingVideoPath('/path/to/video.mov')).toBe(
        '/path/to/video.mov'
      );
    });

    it('getSystemAudioPath uses project folder or legacy fallback', async () => {
      const { getSystemAudioPath } =
        await import('@/main/capture/video/recording-project');
      expect(getSystemAudioPath('/path/to/Rec.poratake/recording.mov')).toBe(
        path.join('/path/to/Rec.poratake', 'system.m4a')
      );
      expect(getSystemAudioPath('/path/to/video.mov')).toBe(
        '/path/to/video.system.m4a'
      );
    });

    it('getMicAudioPath uses project folder or legacy fallback', async () => {
      const { getMicAudioPath } =
        await import('@/main/capture/video/recording-project');
      expect(getMicAudioPath('/path/to/Rec.poratake/recording.mov')).toBe(
        path.join('/path/to/Rec.poratake', 'mic.m4a')
      );
      expect(getMicAudioPath('/path/to/video.mov')).toBe(
        '/path/to/video.mic.m4a'
      );
    });

    it('getCursorPath uses project folder or legacy fallback', async () => {
      const { getCursorPath } =
        await import('@/main/capture/video/recording-project');
      expect(getCursorPath('/path/to/Rec.poratake/recording.mov')).toBe(
        path.join('/path/to/Rec.poratake', 'cursor.json')
      );
      expect(getCursorPath('/path/to/video.mov')).toBe(
        '/path/to/video.cursor.json'
      );
    });

    it('getCameraVideoPath uses project folder or legacy fallback', async () => {
      const { getCameraVideoPath } =
        await import('@/main/capture/video/recording-project');
      expect(getCameraVideoPath('/path/to/Rec.poratake/recording.mov')).toBe(
        path.join('/path/to/Rec.poratake', 'camera.mov')
      );
      expect(getCameraVideoPath('/path/to/video.mov')).toBe(
        '/path/to/video.camera.mov'
      );
    });

    it('getCameraMetaPath uses project folder or legacy fallback', async () => {
      const { getCameraMetaPath } =
        await import('@/main/capture/video/recording-project');
      expect(getCameraMetaPath('/path/to/Rec.poratake/recording.mov')).toBe(
        path.join('/path/to/Rec.poratake', 'camera.json')
      );
      expect(getCameraMetaPath('/path/to/video.mov')).toBe(
        '/path/to/video.camera.json'
      );
    });

    it('getKeysPath uses project folder or legacy fallback', async () => {
      const { getKeysPath } =
        await import('@/main/capture/video/recording-project');
      expect(getKeysPath('/path/to/Rec.poratake/recording.mov')).toBe(
        path.join('/path/to/Rec.poratake', 'keys.json')
      );
      expect(getKeysPath('/path/to/video.mov')).toBe(
        '/path/to/video.keys.json'
      );
    });

    it('getEditorStatePath returns null outside project', async () => {
      const { getEditorStatePath } =
        await import('@/main/capture/video/recording-project');
      expect(getEditorStatePath('/path/to/Rec.poratake/recording.mov')).toBe(
        path.join('/path/to/Rec.poratake', 'state.json')
      );
      expect(getEditorStatePath('/path/to/video.mov')).toBeNull();
    });

    it('getSubtitlePath returns null outside project', async () => {
      const { getSubtitlePath } =
        await import('@/main/capture/video/recording-project');
      expect(getSubtitlePath('/path/to/Rec.poratake/recording.mov')).toBe(
        path.join('/path/to/Rec.poratake', 'subtitle.json')
      );
      expect(getSubtitlePath('/path/to/video.mov')).toBeNull();
    });

    it('getMusicFolderPath returns null outside project', async () => {
      const { getMusicFolderPath } =
        await import('@/main/capture/video/recording-project');
      expect(getMusicFolderPath('/path/to/Rec.poratake/recording.mov')).toBe(
        path.join('/path/to/Rec.poratake', 'music')
      );
      expect(getMusicFolderPath('/path/to/video.mov')).toBeNull();
    });
  });

  describe('createProjectFolder', () => {
    it('throws when path is not a .poratake project', async () => {
      const { createProjectFolder } =
        await import('@/main/capture/video/recording-project');
      expect(() => createProjectFolder('/path/to/video.mov')).toThrow(
        'Project path must end with'
      );
    });

    it('creates folder when missing', async () => {
      const fs = await import('fs');
      vi.mocked(fs.existsSync).mockReturnValue(false);
      const { createProjectFolder } =
        await import('@/main/capture/video/recording-project');
      const result = createProjectFolder('/path/to/My.poratake');
      expect(fs.mkdirSync).toHaveBeenCalledWith('/path/to/My.poratake', {
        recursive: true,
      });
      expect(result).toBe(path.join('/path/to/My.poratake', 'recording.mov'));
    });

    it('does not call mkdir when folder exists', async () => {
      const fs = await import('fs');
      vi.mocked(fs.existsSync).mockReturnValue(true);
      const { createProjectFolder } =
        await import('@/main/capture/video/recording-project');
      createProjectFolder('/path/to/My.poratake');
      expect(fs.mkdirSync).not.toHaveBeenCalled();
    });
  });

  describe('deleteProjectFolder', () => {
    it('removes project folder recursively when path is a project', async () => {
      const fs = await import('fs');
      vi.mocked(fs.existsSync).mockReturnValue(true);
      const { deleteProjectFolder } =
        await import('@/main/capture/video/recording-project');
      await deleteProjectFolder('/path/to/My.poratake/recording.mov');
      expect(fs.rmSync).toHaveBeenCalledWith('/path/to/My.poratake', {
        recursive: true,
        force: true,
      });
    });

    it('unlinks file when not in a project and exists', async () => {
      const fs = await import('fs');
      const fsp = await import('fs/promises');
      vi.mocked(fs.existsSync).mockImplementation(() => true);
      vi.mocked(fsp.default.unlink).mockResolvedValue();
      const { deleteProjectFolder } =
        await import('@/main/capture/video/recording-project');
      await deleteProjectFolder('/path/to/video.mov');
      expect(fsp.default.unlink).toHaveBeenCalledWith('/path/to/video.mov');
    });

    it('does nothing when nothing exists', async () => {
      const fs = await import('fs');
      const fsp = await import('fs/promises');
      vi.mocked(fs.existsSync).mockReturnValue(false);
      const { deleteProjectFolder } =
        await import('@/main/capture/video/recording-project');
      await deleteProjectFolder('/path/to/video.mov');
      expect(fs.rmSync).not.toHaveBeenCalled();
      expect(fsp.default.unlink).not.toHaveBeenCalled();
    });
  });

  describe('deleteRecordingAssets', () => {
    it('deletes every legacy recording asset', async () => {
      const fs = await import('fs');
      vi.mocked(fs.existsSync).mockReturnValue(true);
      const { deleteRecordingAssets } =
        await import('@/main/capture/video/recording-project');

      deleteRecordingAssets('/path/to/video.mov');

      expect(
        vi.mocked(fs.unlinkSync).mock.calls.map(([filePath]) => filePath)
      ).toEqual([
        '/path/to/video.mov',
        '/path/to/video.system.m4a',
        '/path/to/video.mic.m4a',
        '/path/to/video.cursor.json',
        '/path/to/video.mouse.json',
        '/path/to/video.camera.json',
        '/path/to/video.camera.mov',
        '/path/to/video.keys.json',
      ]);
    });

    it('deletes a recording project as one folder', async () => {
      const fs = await import('fs');
      vi.mocked(fs.existsSync).mockReturnValue(true);
      const { deleteRecordingAssets } =
        await import('@/main/capture/video/recording-project');

      deleteRecordingAssets('/path/to/My.poratake/recording.mov');

      expect(fs.rmSync).toHaveBeenCalledWith('/path/to/My.poratake', {
        recursive: true,
        force: true,
      });
      expect(fs.unlinkSync).toHaveBeenCalledWith(
        path.join('/path/to/My.poratake', 'recording.mov')
      );
    });

    it('checks that the project recording can be deleted before its sidecars', async () => {
      const fs = await import('fs');
      vi.mocked(fs.existsSync).mockReturnValue(true);
      vi.mocked(fs.unlinkSync).mockImplementation(() => {
        throw new Error('locked');
      });
      const { deleteRecordingAssets } =
        await import('@/main/capture/video/recording-project');

      expect(() =>
        deleteRecordingAssets('/path/to/My.poratake/recording.mov')
      ).toThrow('locked');
      expect(fs.rmSync).not.toHaveBeenCalled();
    });

    it('does not fail legacy deletion when a sidecar is locked', async () => {
      const fs = await import('fs');
      vi.mocked(fs.existsSync).mockReturnValue(true);
      vi.mocked(fs.unlinkSync).mockImplementation(filePath => {
        if (String(filePath).endsWith('.cursor.json')) {
          throw new Error('locked');
        }
      });
      const warnSpy = vi.spyOn(console, 'warn').mockImplementation(() => {});
      const { deleteRecordingAssets } =
        await import('@/main/capture/video/recording-project');

      expect(() => deleteRecordingAssets('/path/to/video.mov')).not.toThrow();
      expect(fs.unlinkSync).toHaveBeenCalledWith('/path/to/video.mov');
      expect(fs.unlinkSync).toHaveBeenCalledWith('/path/to/video.keys.json');

      warnSpy.mockRestore();
    });

    it('fails legacy deletion when the primary video is locked', async () => {
      const fs = await import('fs');
      vi.mocked(fs.existsSync).mockReturnValue(true);
      vi.mocked(fs.unlinkSync).mockImplementation(filePath => {
        if (filePath === '/path/to/video.mov') {
          throw new Error('locked');
        }
      });
      const { deleteRecordingAssets } =
        await import('@/main/capture/video/recording-project');

      expect(() => deleteRecordingAssets('/path/to/video.mov')).toThrow(
        'locked'
      );
    });
  });

  describe('isValidProject', () => {
    it('returns false when not a .poratake path', async () => {
      const { isValidProject } =
        await import('@/main/capture/video/recording-project');
      expect(isValidProject('/path/to/video.mov')).toBe(false);
    });

    it('returns true when recording.mov exists', async () => {
      const fs = await import('fs');
      vi.mocked(fs.existsSync).mockReturnValue(true);
      const { isValidProject } =
        await import('@/main/capture/video/recording-project');
      expect(isValidProject('/path/to/My.poratake')).toBe(true);
    });

    it('returns false when recording.mov is missing', async () => {
      const fs = await import('fs');
      vi.mocked(fs.existsSync).mockReturnValue(false);
      const { isValidProject } =
        await import('@/main/capture/video/recording-project');
      expect(isValidProject('/path/to/My.poratake')).toBe(false);
    });
  });

  describe('getProjectFiles', () => {
    it('returns empty list when no project folder', async () => {
      const { getProjectFiles } =
        await import('@/main/capture/video/recording-project');
      const result = await getProjectFiles('/path/to/video.mov');
      expect(result).toEqual([]);
    });

    it('returns absolute paths for project files', async () => {
      const fs = await import('fs');
      const fsp = await import('fs/promises');
      vi.mocked(fs.existsSync).mockReturnValue(true);
      vi.mocked(fsp.default.readdir).mockResolvedValue([
        'recording.mov',
        'cursor.json',
      ] as never);
      const { getProjectFiles } =
        await import('@/main/capture/video/recording-project');
      const result = await getProjectFiles('/path/to/My.poratake');
      expect(result).toEqual([
        path.join('/path/to/My.poratake', 'recording.mov'),
        path.join('/path/to/My.poratake', 'cursor.json'),
      ]);
    });

    it('returns empty list when readdir throws', async () => {
      const fs = await import('fs');
      const fsp = await import('fs/promises');
      vi.mocked(fs.existsSync).mockReturnValue(true);
      vi.mocked(fsp.default.readdir).mockRejectedValue(new Error('fail'));
      const { getProjectFiles } =
        await import('@/main/capture/video/recording-project');
      expect(await getProjectFiles('/path/to/My.poratake')).toEqual([]);
    });
  });

  describe('renameRecordingProject', () => {
    it('fails for non-project path', async () => {
      const { renameRecordingProject } =
        await import('@/main/capture/video/recording-project');
      const result = renameRecordingProject('/path/to/video.mov', 'NewName');
      expect(result.success).toBe(false);
      expect(result.error).toBe('Not a valid recording project');
    });

    it('fails when project folder missing', async () => {
      const fs = await import('fs');
      vi.mocked(fs.existsSync).mockReturnValue(false);
      const { renameRecordingProject } =
        await import('@/main/capture/video/recording-project');
      const result = renameRecordingProject('/path/to/My.poratake', 'NewName');
      expect(result.success).toBe(false);
      expect(result.error).toBe('Project folder not found');
    });

    it('fails when sanitized name is empty', async () => {
      const fs = await import('fs');
      vi.mocked(fs.existsSync).mockReturnValue(true);
      const { renameRecordingProject } =
        await import('@/main/capture/video/recording-project');
      const result = renameRecordingProject('/path/to/My.poratake', '   ');
      expect(result.success).toBe(false);
      expect(result.error).toBe('Invalid project name');
    });

    it('returns success without rename if same name', async () => {
      const fs = await import('fs');
      vi.mocked(fs.existsSync).mockReturnValue(true);
      const { renameRecordingProject } =
        await import('@/main/capture/video/recording-project');
      const result = renameRecordingProject(
        path.join('/path/to', 'My.poratake'),
        'My'
      );
      expect(result.success).toBe(true);
      expect(fs.renameSync).not.toHaveBeenCalled();
    });

    it('fails when target already exists', async () => {
      const fs = await import('fs');
      vi.mocked(fs.existsSync).mockImplementation((p: unknown) => {
        return (
          p === '/path/to/My.poratake' ||
          p === path.join('/path/to', 'NewName.poratake')
        );
      });
      const { renameRecordingProject } =
        await import('@/main/capture/video/recording-project');
      const result = renameRecordingProject('/path/to/My.poratake', 'NewName');
      expect(result.success).toBe(false);
      expect(result.error).toBe('A project with this name already exists');
    });

    it('renames successfully', async () => {
      const fs = await import('fs');
      vi.mocked(fs.existsSync).mockImplementation(
        (p: unknown) => p === '/path/to/My.poratake'
      );
      const { renameRecordingProject } =
        await import('@/main/capture/video/recording-project');
      const result = renameRecordingProject('/path/to/My.poratake', 'NewName');
      expect(result.success).toBe(true);
      expect(result.newProjectPath).toBe(
        path.join('/path/to', 'NewName.poratake')
      );
      expect(result.newVideoPath).toBe(
        path.join('/path/to', 'NewName.poratake', 'recording.mov')
      );
      expect(fs.renameSync).toHaveBeenCalledWith(
        '/path/to/My.poratake',
        path.join('/path/to', 'NewName.poratake')
      );
    });

    it('sanitizes invalid characters in name', async () => {
      const fs = await import('fs');
      vi.mocked(fs.existsSync).mockImplementation(
        (p: unknown) => p === '/path/to/My.poratake'
      );
      const { renameRecordingProject } =
        await import('@/main/capture/video/recording-project');
      const result = renameRecordingProject(
        '/path/to/My.poratake',
        'na/me*with?bad'
      );
      expect(result.success).toBe(true);
      expect(result.newProjectPath).toBe(
        path.join('/path/to', 'na-me-with-bad.poratake')
      );
    });

    it('returns error from renameSync exception', async () => {
      const fs = await import('fs');
      vi.mocked(fs.existsSync).mockImplementation(
        (p: unknown) => p === '/path/to/My.poratake'
      );
      vi.mocked(fs.renameSync).mockImplementation(() => {
        throw new Error('disk full');
      });
      const { renameRecordingProject } =
        await import('@/main/capture/video/recording-project');
      const result = renameRecordingProject('/path/to/My.poratake', 'NewName');
      expect(result.success).toBe(false);
      expect(result.error).toBe('disk full');
    });
  });
});
