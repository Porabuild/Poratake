import { describe, it, expect, vi, beforeEach, afterEach } from 'vitest';
import * as os from 'os';
import * as fs from 'fs';
import * as child_process from 'child_process';

vi.mock('os');
vi.mock('fs');
vi.mock('child_process');

describe('Device', () => {
  beforeEach(() => {
    vi.resetModules();
    vi.clearAllMocks();
  });

  afterEach(() => {
    vi.restoreAllMocks();
  });

  describe('getMachineId', () => {
    it('should return hardware UUID on macOS', async () => {
      vi.mocked(os.platform).mockReturnValue('darwin');
      vi.mocked(child_process.execSync).mockReturnValue(
        Buffer.from('ABC123-DEF456-GHI789\n')
      );

      const { getMachineId } = await import('@/main/license/device');
      const result = getMachineId();

      expect(result).toBe('ABC123-DEF456-GHI789');
      expect(child_process.execSync).toHaveBeenCalledWith(
        expect.stringContaining('ioreg')
      );
    });

    it('should return machine GUID on Windows', async () => {
      vi.mocked(os.platform).mockReturnValue('win32');
      vi.mocked(child_process.execSync).mockReturnValue(
        Buffer.from('MachineGuid    REG_SZ    12345678-ABCD-EFGH\n')
      );

      const { getMachineId } = await import('@/main/license/device');
      const result = getMachineId();

      expect(result).toBe('12345678-ABCD-EFGH');
    });

    it('should return fallback when Windows GUID not found', async () => {
      vi.mocked(os.platform).mockReturnValue('win32');
      vi.mocked(os.hostname).mockReturnValue('test-host');
      vi.mocked(child_process.execSync).mockReturnValue(
        Buffer.from('No GUID found\n')
      );

      const { getMachineId } = await import('@/main/license/device');
      const result = getMachineId();

      expect(result).toBe('test-host');
    });

    it('should return machine-id on Linux', async () => {
      vi.mocked(os.platform).mockReturnValue('linux');
      vi.mocked(fs.existsSync).mockReturnValue(true);
      vi.mocked(fs.readFileSync).mockReturnValue('linux-machine-id-12345\n');

      const { getMachineId } = await import('@/main/license/device');
      const result = getMachineId();

      expect(result).toBe('linux-machine-id-12345');
      expect(fs.readFileSync).toHaveBeenCalledWith('/etc/machine-id', 'utf8');
    });

    it('should return fallback on Linux when machine-id does not exist', async () => {
      vi.mocked(os.platform).mockReturnValue('linux');
      vi.mocked(fs.existsSync).mockReturnValue(false);
      vi.mocked(os.hostname).mockReturnValue('linux-host');
      vi.mocked(os.arch).mockReturnValue('x64');

      const { getMachineId } = await import('@/main/license/device');
      const result = getMachineId();

      expect(result).toBe('linux-host-linux-x64');
    });

    it('should return fallback on error', async () => {
      vi.mocked(os.platform).mockReturnValue('darwin');
      vi.mocked(child_process.execSync).mockImplementation(() => {
        throw new Error('Command failed');
      });
      vi.mocked(os.hostname).mockReturnValue('fallback-host');
      vi.mocked(os.arch).mockReturnValue('arm64');

      const { getMachineId } = await import('@/main/license/device');
      const result = getMachineId();

      expect(result).toBe('fallback-host-darwin-arm64');
    });
  });

  describe('generateDeviceFingerprint', () => {
    it('should generate a 32-character fingerprint', async () => {
      vi.mocked(os.platform).mockReturnValue('darwin');
      vi.mocked(child_process.execSync).mockReturnValue(
        Buffer.from('TEST-UUID-123\n')
      );

      const { generateDeviceFingerprint } =
        await import('@/main/license/device');
      const result = generateDeviceFingerprint();

      expect(result).toHaveLength(32);
      expect(result).toMatch(/^[a-f0-9]{32}$/);
    });

    it('should generate consistent fingerprint for same machine', async () => {
      vi.mocked(os.platform).mockReturnValue('darwin');
      vi.mocked(child_process.execSync).mockReturnValue(
        Buffer.from('CONSISTENT-UUID\n')
      );

      const { generateDeviceFingerprint } =
        await import('@/main/license/device');
      const result1 = generateDeviceFingerprint();
      const result2 = generateDeviceFingerprint();

      expect(result1).toBe(result2);
    });

    it('should generate different fingerprints for different machines', async () => {
      vi.mocked(os.platform).mockReturnValue('darwin');

      vi.mocked(child_process.execSync).mockReturnValue(
        Buffer.from('MACHINE-1\n')
      );
      const { generateDeviceFingerprint: fp1 } =
        await import('@/main/license/device');
      const result1 = fp1();

      vi.resetModules();

      vi.mocked(child_process.execSync).mockReturnValue(
        Buffer.from('MACHINE-2\n')
      );
      const { generateDeviceFingerprint: fp2 } =
        await import('@/main/license/device');
      const result2 = fp2();

      expect(result1).not.toBe(result2);
    });
  });

  describe('getDeviceName', () => {
    it('should return hostname without .local suffix', async () => {
      vi.mocked(os.hostname).mockReturnValue('MacBook-Pro.local');

      const { getDeviceName } = await import('@/main/license/device');
      const result = getDeviceName();

      expect(result).toBe('MacBook-Pro');
    });

    it('should return hostname as-is when no .local suffix', async () => {
      vi.mocked(os.hostname).mockReturnValue('my-computer');

      const { getDeviceName } = await import('@/main/license/device');
      const result = getDeviceName();

      expect(result).toBe('my-computer');
    });
  });

  describe('getDevicePlatform', () => {
    it('should return macOS for darwin', async () => {
      vi.mocked(os.platform).mockReturnValue('darwin');

      const { getDevicePlatform } = await import('@/main/license/device');
      const result = getDevicePlatform();

      expect(result).toBe('macOS');
    });

    it('should return Windows for win32', async () => {
      vi.mocked(os.platform).mockReturnValue('win32');

      const { getDevicePlatform } = await import('@/main/license/device');
      const result = getDevicePlatform();

      expect(result).toBe('Windows');
    });

    it('should return Linux for linux', async () => {
      vi.mocked(os.platform).mockReturnValue('linux');

      const { getDevicePlatform } = await import('@/main/license/device');
      const result = getDevicePlatform();

      expect(result).toBe('Linux');
    });

    it('should return platform name for unknown platforms', async () => {
      vi.mocked(os.platform).mockReturnValue('freebsd' as NodeJS.Platform);

      const { getDevicePlatform } = await import('@/main/license/device');
      const result = getDevicePlatform();

      expect(result).toBe('freebsd');
    });
  });
});
