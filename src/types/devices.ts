export interface MediaDeviceDescriptor {
  id: string;
  label: string;
}

export interface MediaDeviceLists {
  microphones: MediaDeviceDescriptor[];
  cameras: MediaDeviceDescriptor[];
}

export interface DeviceTestTarget {
  deviceId: string | null;
  deviceName: string | null;
  flipped?: boolean;
}
