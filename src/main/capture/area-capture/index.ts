import type { Rectangle } from 'electron';
import { getConfig } from '@/main/settings';
import { startAreaSelection } from '@/main/capture/area-selector';
import {
  freezeScreen,
  releaseScreen,
  isSupported as isFreezeScreenSupported,
} from '@/main/capture/freeze-screen';
import { captureRegionToFile } from '@/main/capture/screenshot/native-capture';

interface AreaSelectionSession {
  rect: Rectangle;
  frozen: boolean;
  release: () => Promise<void>;
}

async function beginAreaSelection(): Promise<AreaSelectionSession | null> {
  const shouldFreeze =
    getConfig().screenshot.freezeScreen && isFreezeScreenSupported();
  const frozen = shouldFreeze ? await freezeScreen() : false;

  const release = async () => {
    if (frozen) {
      await releaseScreen();
    }
  };

  const selection = await startAreaSelection({
    autoConfirm: true,
    showPrompt: true,
  });

  if (
    !selection ||
    selection.x === undefined ||
    selection.y === undefined ||
    selection.width === undefined ||
    selection.height === undefined
  ) {
    await release();
    return null;
  }

  return {
    rect: {
      x: selection.x,
      y: selection.y,
      width: selection.width,
      height: selection.height,
    },
    frozen,
    release,
  };
}

export async function selectAreaRegion(): Promise<Rectangle | null> {
  const session = await beginAreaSelection();
  if (!session) {
    return null;
  }

  await session.release();
  return session.rect;
}

export async function captureAreaToFile(filePath: string): Promise<boolean> {
  const session = await beginAreaSelection();
  if (!session) {
    return false;
  }

  try {
    return await captureRegionToFile(session.rect, filePath, {
      frozen: session.frozen,
    });
  } finally {
    await session.release();
  }
}
