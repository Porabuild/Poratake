import { useEffect, useRef, useState, useCallback } from 'react';

interface UseAudioLevelOptions {
  deviceId: string | null;
  enabled: boolean;
  smoothingFactor?: number;
}

export function useAudioLevel({
  deviceId,
  enabled,
  smoothingFactor = 0.3,
}: UseAudioLevelOptions) {
  const [level, setLevel] = useState(0);
  const streamRef = useRef<MediaStream | null>(null);
  const audioContextRef = useRef<AudioContext | null>(null);
  const analyserRef = useRef<AnalyserNode | null>(null);
  const animationFrameRef = useRef<number | null>(null);
  const smoothedLevelRef = useRef(0);

  const cleanup = useCallback(() => {
    if (animationFrameRef.current) {
      cancelAnimationFrame(animationFrameRef.current);
      animationFrameRef.current = null;
    }
    if (streamRef.current) {
      streamRef.current.getTracks().forEach(track => track.stop());
      streamRef.current = null;
    }
    if (audioContextRef.current) {
      audioContextRef.current.close();
      audioContextRef.current = null;
    }
    analyserRef.current = null;
    smoothedLevelRef.current = 0;
    setLevel(0);
  }, []);

  useEffect(() => {
    if (!enabled || !deviceId) {
      cleanup();
      return;
    }

    let mounted = true;

    const startMonitoring = async () => {
      try {
        const stream = await navigator.mediaDevices.getUserMedia({
          audio: { deviceId: { exact: deviceId } },
        });

        if (!mounted) {
          stream.getTracks().forEach(track => track.stop());
          return;
        }

        streamRef.current = stream;

        const audioContext = new AudioContext();
        audioContextRef.current = audioContext;

        const source = audioContext.createMediaStreamSource(stream);
        const analyser = audioContext.createAnalyser();
        analyser.fftSize = 256;
        analyser.smoothingTimeConstant = 0.5;
        source.connect(analyser);
        analyserRef.current = analyser;

        const dataArray = new Uint8Array(analyser.frequencyBinCount);

        const updateLevel = () => {
          if (!mounted || !analyserRef.current) return;

          analyserRef.current.getByteFrequencyData(dataArray);

          let sum = 0;
          for (let i = 0; i < dataArray.length; i++) {
            sum += dataArray[i] * dataArray[i];
          }
          const rms = Math.sqrt(sum / dataArray.length);

          const normalizedLevel = Math.min(1, (rms / 128) * 1.5);

          smoothedLevelRef.current =
            smoothedLevelRef.current * (1 - smoothingFactor) +
            normalizedLevel * smoothingFactor;

          setLevel(smoothedLevelRef.current);

          animationFrameRef.current = requestAnimationFrame(updateLevel);
        };

        updateLevel();
      } catch (error) {
        console.error('Failed to start audio monitoring:', error);
      }
    };

    startMonitoring();

    return () => {
      mounted = false;
      cleanup();
    };
  }, [deviceId, enabled, smoothingFactor, cleanup]);

  return level;
}
