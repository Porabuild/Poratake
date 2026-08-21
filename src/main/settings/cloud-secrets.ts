import { safeStorage } from 'electron';
import type { CloudConfig } from '@/types/settings.ts';

const PROTECTED_VALUE_PREFIX = 'poratake-safe-storage:v1:';

interface RetainedSecret {
  storedValue: string;
  revealedValue: string | null;
}

export interface RetainedCloudSecrets {
  s3SecretAccessKey?: RetainedSecret;
  restHeaders: Array<
    | {
        key: string;
        secret: RetainedSecret;
      }
    | undefined
  >;
}

interface CloudSecretsResult {
  cloud: CloudConfig;
  retained: RetainedCloudSecrets;
  failedToProtect: boolean;
}

function revealValue(value: string): {
  value: string;
  retained?: RetainedSecret;
} {
  if (!value) {
    return { value };
  }
  if (!value.startsWith(PROTECTED_VALUE_PREFIX)) {
    return {
      value,
      retained: { storedValue: value, revealedValue: value },
    };
  }

  if (!safeStorage.isEncryptionAvailable()) {
    console.error(
      'Failed to reveal cloud credential: OS encryption unavailable'
    );
    return {
      value: '',
      retained: { storedValue: value, revealedValue: null },
    };
  }

  try {
    const encrypted = Buffer.from(
      value.slice(PROTECTED_VALUE_PREFIX.length),
      'base64'
    );
    const revealedValue = safeStorage.decryptString(encrypted);
    return {
      value: revealedValue,
      retained: { storedValue: value, revealedValue },
    };
  } catch (error) {
    console.error('Failed to reveal cloud credential:', error);
    return {
      value: '',
      retained: { storedValue: value, revealedValue: null },
    };
  }
}

function protectValue(
  value: string,
  retained?: RetainedSecret
): { value: string; retained?: RetainedSecret; failedToProtect: boolean } {
  if (!value) {
    if (retained?.revealedValue === null) {
      return {
        value: retained.storedValue,
        retained,
        failedToProtect: false,
      };
    }
    return { value, failedToProtect: false };
  }

  if (safeStorage.isEncryptionAvailable()) {
    try {
      const storedValue = `${PROTECTED_VALUE_PREFIX}${safeStorage.encryptString(value).toString('base64')}`;
      return {
        value: storedValue,
        retained: { storedValue, revealedValue: value },
        failedToProtect: false,
      };
    } catch (error) {
      console.error('Failed to protect cloud credential:', error);
    }
  } else {
    console.error(
      'Failed to protect cloud credential: OS encryption unavailable'
    );
  }

  if (retained) {
    return {
      value: retained.storedValue,
      retained,
      failedToProtect: retained.revealedValue !== value,
    };
  }
  return { value: '', failedToProtect: true };
}

export function hasUnprotectedCloudSecrets(cloud: CloudConfig): boolean {
  if (
    cloud.s3.secretAccessKey &&
    !cloud.s3.secretAccessKey.startsWith(PROTECTED_VALUE_PREFIX)
  ) {
    return true;
  }
  return cloud.rest.headers.some(
    header => header.value && !header.value.startsWith(PROTECTED_VALUE_PREFIX)
  );
}

export function protectCloudSecrets(
  cloud: CloudConfig,
  retained: RetainedCloudSecrets
): CloudSecretsResult {
  const s3SecretAccessKey = protectValue(
    cloud.s3.secretAccessKey,
    retained.s3SecretAccessKey
  );
  const usedRetainedHeaders = new Set<number>();
  const restHeaders = cloud.rest.headers.map((header, index) => {
    let retainedIndex =
      retained.restHeaders[index]?.key === header.key ? index : -1;
    if (retainedIndex < 0 || usedRetainedHeaders.has(retainedIndex)) {
      retainedIndex = retained.restHeaders.findIndex(
        (candidate, candidateIndex) =>
          candidate?.key === header.key &&
          !usedRetainedHeaders.has(candidateIndex)
      );
    }
    if (
      retainedIndex < 0 &&
      header.value === '' &&
      retained.restHeaders[index]?.secret.revealedValue === null &&
      !usedRetainedHeaders.has(index)
    ) {
      retainedIndex = index;
    }
    if (retainedIndex >= 0) {
      usedRetainedHeaders.add(retainedIndex);
    }
    const retainedHeader = retained.restHeaders[retainedIndex];
    return protectValue(header.value, retainedHeader?.secret);
  });

  return {
    cloud: {
      ...cloud,
      s3: {
        ...cloud.s3,
        secretAccessKey: s3SecretAccessKey.value,
      },
      rest: {
        ...cloud.rest,
        headers: cloud.rest.headers.map((header, index) => ({
          ...header,
          value: restHeaders[index].value,
        })),
      },
    },
    retained: {
      s3SecretAccessKey: s3SecretAccessKey.retained,
      restHeaders: restHeaders.map((header, index) =>
        header.retained
          ? { key: cloud.rest.headers[index].key, secret: header.retained }
          : undefined
      ),
    },
    failedToProtect:
      s3SecretAccessKey.failedToProtect ||
      restHeaders.some(header => header.failedToProtect),
  };
}

export function revealCloudSecrets(cloud: CloudConfig): CloudSecretsResult {
  const s3SecretAccessKey = revealValue(cloud.s3.secretAccessKey);
  const restHeaders = cloud.rest.headers.map(header =>
    revealValue(header.value)
  );

  return {
    cloud: {
      ...cloud,
      s3: {
        ...cloud.s3,
        secretAccessKey: s3SecretAccessKey.value,
      },
      rest: {
        ...cloud.rest,
        headers: cloud.rest.headers.map((header, index) => ({
          ...header,
          value: restHeaders[index].value,
        })),
      },
    },
    retained: {
      s3SecretAccessKey: s3SecretAccessKey.retained,
      restHeaders: restHeaders.map((header, index) =>
        header.retained
          ? { key: cloud.rest.headers[index].key, secret: header.retained }
          : undefined
      ),
    },
    failedToProtect: false,
  };
}
