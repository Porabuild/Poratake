import { describe, it, expect, vi, beforeEach } from 'vitest';

vi.mock('fs', () => ({
  existsSync: vi.fn(),
  mkdirSync: vi.fn(),
  rmSync: vi.fn(),
  renameSync: vi.fn(),
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
    it('returns true when path ends with .capty', async () => {
      const { isRecordingProject } =
        await import('@/main/capture/video/recording-project');
      expect(isRecordingProject('/path/to/My Recording.capty')).toBe(true);
    });

    it('returns false when path does not end with .capty', async () => {
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
      expect(getProjectFolder('/path/to/Rec.capty')).toBe('/path/to/Rec.capty');
    });

    it('returns parent dir when input is a file inside a project', async () => {
      const { getProjectFolder } =
        await import('@/main/capture/video/recording-project');
      expect(getProjectFolder('/path/to/Rec.capty/recording.mov')).toBe(
        '/path/to/Rec.capty'
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
      expect(getRecordingVideoPath('/path/to/Rec.capty')).toBe(
        '/path/to/Rec.capty/recording.mov'
      );
      expect(getRecordingVideoPath('/path/to/video.mov')).toBe(
        '/path/to/video.mov'
      );
    });

    it('getSystemAudioPath uses project folder or legacy fallback', async () => {
      const { getSystemAudioPath } =
        await import('@/main/capture/video/recording-project');
      expect(getSystemAudioPath('/path/to/Rec.capty/recording.mov')).toBe(
        '/path/to/Rec.capty/system.m4a'
      );
      expect(getSystemAudioPath('/path/to/video.mov')).toBe(
        '/path/to/video.system.m4a'
      );
    });

    it('getMicAudioPath uses project folder or legacy fallback', async () => {
      const { getMicAudioPath } =
        await import('@/main/capture/video/recording-project');
      expect(getMicAudioPath('/path/to/Rec.capty/recording.mov')).toBe(
        '/path/to/Rec.capty/mic.m4a'
      );
      expect(getMicAudioPath('/path/to/video.mov')).toBe(
        '/path/to/video.mic.m4a'
      );
    });

    it('getCursorPath uses project folder or legacy fallback', async () => {
      const { getCursorPath } =
        await import('@/main/capture/video/recording-project');
      expect(getCursorPath('/path/to/Rec.capty/recording.mov')).toBe(
        '/path/to/Rec.capty/cursor.json'
      );
      expect(getCursorPath('/path/to/video.mov')).toBe(
        '/path/to/video.cursor.json'
      );
    });

    it('getCameraVideoPath uses project folder or legacy fallback', async () => {
      const { getCameraVideoPath } =
        await import('@/main/capture/video/recording-project');
      expect(getCameraVideoPath('/path/to/Rec.capty/recording.mov')).toBe(
        '/path/to/Rec.capty/camera.mov'
      );
      expect(getCameraVideoPath('/path/to/video.mov')).toBe(
        '/path/to/video.camera.mov'
      );
    });

    it('getCameraMetaPath uses project folder or legacy fallback', async () => {
      const { getCameraMetaPath } =
        await import('@/main/capture/video/recording-project');
      expect(getCameraMetaPath('/path/to/Rec.capty/recording.mov')).toBe(
        '/path/to/Rec.capty/camera.json'
      );
      expect(getCameraMetaPath('/path/to/video.mov')).toBe(
        '/path/to/video.camera.json'
      );
    });

    it('getKeysPath uses project folder or legacy fallback', async () => {
      const { getKeysPath } =
        await import('@/main/capture/video/recording-project');
      expect(getKeysPath('/path/to/Rec.capty/recording.mov')).toBe(
        '/path/to/Rec.capty/keys.json'
      );
      expect(getKeysPath('/path/to/video.mov')).toBe(
        '/path/to/video.keys.json'
      );
    });

    it('getEditorStatePath returns null outside project', async () => {
      const { getEditorStatePath } =
        await import('@/main/capture/video/recording-project');
      expect(getEditorStatePath('/path/to/Rec.capty/recording.mov')).toBe(
        '/path/to/Rec.capty/state.json'
      );
      expect(getEditorStatePath('/path/to/video.mov')).toBeNull();
    });

    it('getSubtitlePath returns null outside project', async () => {
      const { getSubtitlePath } =
        await import('@/main/capture/video/recording-project');
      expect(getSubtitlePath('/path/to/Rec.capty/recording.mov')).toBe(
        '/path/to/Rec.capty/subtitle.json'
      );
      expect(getSubtitlePath('/path/to/video.mov')).toBeNull();
    });

    it('getMusicFolderPath returns null outside project', async () => {
      const { getMusicFolderPath } =
        await import('@/main/capture/video/recording-project');
      expect(getMusicFolderPath('/path/to/Rec.capty/recording.mov')).toBe(
        '/path/to/Rec.capty/music'
      );
      expect(getMusicFolderPath('/path/to/video.mov')).toBeNull();
    });
  });

  describe('createProjectFolder', () => {
    it('throws when path is not a .capty project', async () => {
      const { createProjectFolder } =
        await import('@/main/capture/video/recording-project');
      expect(() => createProjectFolder('/path/to/video.mov')).toThrow();
    });

    it('creates folder when missing', async () => {
      const fs = await import('fs');
      vi.mocked(fs.existsSync).mockReturnValue(false);
      const { createProjectFolder } =
        await import('@/main/capture/video/recording-project');
      const result = createProjectFolder('/path/to/My.capty');
      expect(fs.mkdirSync).toHaveBeenCalledWith('/path/to/My.capty', {
        recursive: true,
      });
      expect(result).toBe('/path/to/My.capty/recording.mov');
    });

    it('does not call mkdir when folder exists', async () => {
      const fs = await import('fs');
      vi.mocked(fs.existsSync).mockReturnValue(true);
      const { createProjectFolder } =
        await import('@/main/capture/video/recording-project');
      createProjectFolder('/path/to/My.capty');
      expect(fs.mkdirSync).not.toHaveBeenCalled();
    });
  });

  describe('deleteProjectFolder', () => {
    it('removes project folder recursively when path is a project', async () => {
      const fs = await import('fs');
      vi.mocked(fs.existsSync).mockReturnValue(true);
      const { deleteProjectFolder } =
        await import('@/main/capture/video/recording-project');
      await deleteProjectFolder('/path/to/My.capty/recording.mov');
      expect(fs.rmSync).toHaveBeenCalledWith('/path/to/My.capty', {
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

  describe('isValidProject', () => {
    it('returns false when not a .capty path', async () => {
      const { isValidProject } =
        await import('@/main/capture/video/recording-project');
      expect(isValidProject('/path/to/video.mov')).toBe(false);
    });

    it('returns true when recording.mov exists', async () => {
      const fs = await import('fs');
      vi.mocked(fs.existsSync).mockReturnValue(true);
      const { isValidProject } =
        await import('@/main/capture/video/recording-project');
      expect(isValidProject('/path/to/My.capty')).toBe(true);
    });

    it('returns false when recording.mov is missing', async () => {
      const fs = await import('fs');
      vi.mocked(fs.existsSync).mockReturnValue(false);
      const { isValidProject } =
        await import('@/main/capture/video/recording-project');
      expect(isValidProject('/path/to/My.capty')).toBe(false);
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
      const result = await getProjectFiles('/path/to/My.capty');
      expect(result).toEqual([
        '/path/to/My.capty/recording.mov',
        '/path/to/My.capty/cursor.json',
      ]);
    });

    it('returns empty list when readdir throws', async () => {
      const fs = await import('fs');
      const fsp = await import('fs/promises');
      vi.mocked(fs.existsSync).mockReturnValue(true);
      vi.mocked(fsp.default.readdir).mockRejectedValue(new Error('fail'));
      const { getProjectFiles } =
        await import('@/main/capture/video/recording-project');
      expect(await getProjectFiles('/path/to/My.capty')).toEqual([]);
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
      const result = renameRecordingProject('/path/to/My.capty', 'NewName');
      expect(result.success).toBe(false);
      expect(result.error).toBe('Project folder not found');
    });

    it('fails when sanitized name is empty', async () => {
      const fs = await import('fs');
      vi.mocked(fs.existsSync).mockReturnValue(true);
      const { renameRecordingProject } =
        await import('@/main/capture/video/recording-project');
      const result = renameRecordingProject('/path/to/My.capty', '   ');
      expect(result.success).toBe(false);
      expect(result.error).toBe('Invalid project name');
    });

    it('returns success without rename if same name', async () => {
      const fs = await import('fs');
      vi.mocked(fs.existsSync).mockReturnValue(true);
      const { renameRecordingProject } =
        await import('@/main/capture/video/recording-project');
      const result = renameRecordingProject('/path/to/My.capty', 'My');
      expect(result.success).toBe(true);
      expect(fs.renameSync).not.toHaveBeenCalled();
    });

    it('fails when target already exists', async () => {
      const fs = await import('fs');
      vi.mocked(fs.existsSync).mockImplementation((p: unknown) => {
        return p === '/path/to/My.capty' || p === '/path/to/NewName.capty';
      });
      const { renameRecordingProject } =
        await import('@/main/capture/video/recording-project');
      const result = renameRecordingProject('/path/to/My.capty', 'NewName');
      expect(result.success).toBe(false);
      expect(result.error).toBe('A project with this name already exists');
    });

    it('renames successfully', async () => {
      const fs = await import('fs');
      vi.mocked(fs.existsSync).mockImplementation(
        (p: unknown) => p === '/path/to/My.capty'
      );
      const { renameRecordingProject } =
        await import('@/main/capture/video/recording-project');
      const result = renameRecordingProject('/path/to/My.capty', 'NewName');
      expect(result.success).toBe(true);
      expect(result.newProjectPath).toBe('/path/to/NewName.capty');
      expect(result.newVideoPath).toBe('/path/to/NewName.capty/recording.mov');
      expect(fs.renameSync).toHaveBeenCalledWith(
        '/path/to/My.capty',
        '/path/to/NewName.capty'
      );
    });

    it('sanitizes invalid characters in name', async () => {
      const fs = await import('fs');
      vi.mocked(fs.existsSync).mockImplementation(
        (p: unknown) => p === '/path/to/My.capty'
      );
      const { renameRecordingProject } =
        await import('@/main/capture/video/recording-project');
      const result = renameRecordingProject(
        '/path/to/My.capty',
        'na/me*with?bad'
      );
      expect(result.success).toBe(true);
      expect(result.newProjectPath).toBe('/path/to/na-me-with-bad.capty');
    });

    it('returns error from renameSync exception', async () => {
      const fs = await import('fs');
      vi.mocked(fs.existsSync).mockImplementation(
        (p: unknown) => p === '/path/to/My.capty'
      );
      vi.mocked(fs.renameSync).mockImplementation(() => {
        throw new Error('disk full');
      });
      const { renameRecordingProject } =
        await import('@/main/capture/video/recording-project');
      const result = renameRecordingProject('/path/to/My.capty', 'NewName');
      expect(result.success).toBe(false);
      expect(result.error).toBe('disk full');
    });
  });
});
