import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { useCallback, useEffect, useRef, useState } from "react";

const STORAGE_KEY = "safeclaw-tts-enabled";

export interface TtsModelStatus {
	zh_ready: boolean;
	en_ready: boolean;
}

interface TtsDownloadProgress {
	model_index: number;
	total_models: number;
	percent: number;
	lang: string;
}

export function useTts() {
	const [ttsEnabled, setTtsEnabledState] = useState(
		() => localStorage.getItem(STORAGE_KEY) === "true",
	);
	const [isSpeaking, setIsSpeaking] = useState(false);
	const [modelStatus, setModelStatus] = useState<TtsModelStatus | null>(null);
	const [isDownloading, setIsDownloading] = useState(false);
	const [downloadProgress, setDownloadProgress] =
		useState<TtsDownloadProgress | null>(null);
	const speakingRef = useRef(false);

	const setTtsEnabled = useCallback((enabled: boolean) => {
		setTtsEnabledState(enabled);
		localStorage.setItem(STORAGE_KEY, enabled ? "true" : "false");
	}, []);

	// Check model status on mount
	useEffect(() => {
		invoke<TtsModelStatus>("voice_tts_status")
			.then(setModelStatus)
			.catch(() => setModelStatus({ zh_ready: false, en_ready: false }));
	}, []);

	// Listen for download progress events
	useEffect(() => {
		const unlisten = listen<TtsDownloadProgress>(
			"voice://tts-download-progress",
			(event) => {
				setDownloadProgress(event.payload);
			},
		);
		return () => {
			unlisten.then((fn) => fn());
		};
	}, []);

	const downloadModels = useCallback(async () => {
		setIsDownloading(true);
		setDownloadProgress(null);
		try {
			await invoke("voice_tts_download");
			const status = await invoke<TtsModelStatus>("voice_tts_status");
			setModelStatus(status);
		} catch (e) {
			console.error("TTS model download failed:", e);
		} finally {
			setIsDownloading(false);
			setDownloadProgress(null);
		}
	}, []);

	const speak = useCallback(async (text: string) => {
		if (!text.trim()) return;
		try {
			speakingRef.current = true;
			setIsSpeaking(true);
			await invoke("voice_tts_speak", { text });
		} catch (e) {
			console.error("TTS speak failed:", e);
		}
		// Note: we can't know exactly when playback finishes from the Rust side,
		// so we estimate based on text length (~150 chars/sec for Chinese, ~200 for English)
		const estimatedMs = Math.max(1000, text.length * 80);
		setTimeout(() => {
			if (speakingRef.current) {
				speakingRef.current = false;
				setIsSpeaking(false);
			}
		}, estimatedMs);
	}, []);

	const stop = useCallback(async () => {
		speakingRef.current = false;
		setIsSpeaking(false);
		try {
			await invoke("voice_tts_stop");
		} catch (e) {
			console.error("TTS stop failed:", e);
		}
	}, []);

	const modelsReady = modelStatus
		? modelStatus.zh_ready && modelStatus.en_ready
		: false;

	return {
		ttsEnabled,
		setTtsEnabled,
		isSpeaking,
		modelsReady,
		modelStatus,
		isDownloading,
		downloadProgress,
		downloadModels,
		speak,
		stop,
	};
}
