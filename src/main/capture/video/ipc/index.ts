import { registerDialogHandlers } from './dialog-handlers';
import { registerDataHandlers } from './data-handlers';
import { registerStateHandlers } from './state-handlers';
import { registerAudioHandlers } from './audio-handlers';
import { registerExportHandlers } from './export-handlers';
import { registerFileHandlers } from './file-handlers';
import { registerSubtitleHandlers } from './subtitle-handlers';
import { registerMetadataHandlers } from './metadata-handlers';
import { registerKeyboardSoundHandlers } from './keyboard-sound-handlers';
import { registerProjectHandlers } from './project-handlers';
import { registerMusicHandlers } from './music-handlers';
import { registerExportSessionHandlers } from './export-session';

export function registerAllVideoEditorHandlers(): void {
  registerExportSessionHandlers();
  registerDialogHandlers();
  registerDataHandlers();
  registerStateHandlers();
  registerAudioHandlers();
  registerExportHandlers();
  registerFileHandlers();
  registerSubtitleHandlers();
  registerMetadataHandlers();
  registerKeyboardSoundHandlers();
  registerProjectHandlers();
  registerMusicHandlers();
}

export {
  registerDialogHandlers,
  registerDataHandlers,
  registerStateHandlers,
  registerAudioHandlers,
  registerExportHandlers,
  registerFileHandlers,
  registerSubtitleHandlers,
  registerMetadataHandlers,
  registerKeyboardSoundHandlers,
  registerProjectHandlers,
  registerMusicHandlers,
};
